//! # eulerphi-core
//!
//! Everything that decides whether a credential counts lives here, so the SP1
//! guest and the native host run byte-for-byte the same logic.
//!
//! Pipeline for each Reclaim zkTLS proof:
//! 1. recompute the claim identifier and recover the attestor signatures;
//! 2. require at least one trusted attestor;
//! 3. require the attested URL to be an allowed Udemy/Coursera endpoint;
//! 4. require the proof to be bound to the prover's wallet and to our app tag;
//! 5. require the attestation to be fresh;
//! 6. extract title / completion / certificate id.
//!
//! Then across the set: dedup, match the Phi industry, count, commit.

pub mod error;
pub mod policy;
pub mod reclaim;

use std::collections::BTreeMap;

use alloy_primitives::{Address, FixedBytes};
use alloy_sol_types::{sol, SolValue};
use sha2::{Digest, Sha256};

pub use error::VerifyError;
pub use policy::{Industry, PlatformRule, Policy};
pub use reclaim::{ClaimData, ReclaimProof};

sol! {
    /// ABI-encoded public output of the SP1 program. This is ALL a verifier learns.
    #[derive(Debug, PartialEq)]
    struct PublicValuesStruct {
        /// Wallet / Phi identity the credentials are bound to.
        address wallet;
        /// sha256 of the policy (attestors, endpoints, industry keywords, thresholds).
        bytes32 policyHash;
        /// Phi industry id, e.g. "crypto".
        string industry;
        uint32 minCount;
        /// Distinct credentials that satisfied the policy.
        uint32 qualifiedCount;
        bool passed;
        /// sha256 over the sorted dedup keys of the qualifying credentials.
        /// Lets the holder later reveal a specific credential without re-proving.
        bytes32 certSetCommitment;
        /// Unix time the policy was evaluated at (freshness reference).
        uint64 evaluatedAt;
    }
}

/// Everything the SP1 program reads from stdin.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProgramInput {
    pub proofs: Vec<ReclaimProof>,
    pub policy: Policy,
    /// `0x`-prefixed wallet address the proofs must be bound to.
    pub wallet: String,
    /// Evaluation time (unix seconds), supplied by the host and made public.
    pub evaluated_at: u64,
}

/// A credential extracted from one verified proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credential {
    pub platform: String,
    pub title: String,
    pub completion_pct: u32,
    pub cert_id: Option<String>,
    pub attested_at: u64,
}

impl Credential {
    /// Stable identity of a credential, used for deduplication and the commitment.
    pub fn dedup_key(&self) -> [u8; 32] {
        let id = match &self.cert_id {
            Some(c) if !c.trim().is_empty() => format!("id:{}", c.trim()),
            _ => format!("title:{}", normalize_title(&self.title)),
        };
        let mut h = Sha256::new();
        h.update(self.platform.as_bytes());
        h.update([0u8]);
        h.update(id.as_bytes());
        h.finalize().into()
    }
}

fn normalize_title(t: &str) -> String {
    t.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Host-side view of an evaluation (the guest only commits `public`).
#[derive(Debug)]
pub struct Evaluation {
    pub public: PublicValuesStruct,
    /// Every verified credential and whether it counted.
    pub credentials: Vec<(Credential, Verdict)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Counted,
    Duplicate,
    OtherIndustry,
    Incomplete,
}

/// `https://host/path?query` -> (host, path). Only https is accepted.
fn split_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].to_lowercase();
    let path = rest[end..].to_string();
    let path = if path.is_empty() { "/".to_string() } else { path };
    Some((host, path))
}

/// Extracted parameters: `context.extractedParameters` (what the attestor pulled
/// out of the response), falling back to `parameters.paramValues`.
fn extracted_params(
    ctx: &serde_json::Value,
    params: &serde_json::Value,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for src in [params.get("paramValues"), ctx.get("extractedParameters")] {
        if let Some(obj) = src.and_then(|v| v.as_object()) {
            for (k, v) in obj {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                out.insert(k.clone(), s);
            }
        }
    }
    out
}

fn first_of<'a>(map: &'a BTreeMap<String, String>, keys: &[String]) -> Option<&'a String> {
    keys.iter().find_map(|k| map.get(k))
}

fn parse_completion(raw: &str, fraction: bool) -> Option<u32> {
    let x: f64 = raw.trim().trim_end_matches('%').trim().parse().ok()?;
    let pct = if fraction { x * 100.0 } else { x };
    if !(0.0..=100.0001).contains(&pct) {
        return None;
    }
    Some(pct.floor() as u32)
}

/// Verify one proof against the policy and extract its credential.
pub fn verify_proof(
    proof: &ReclaimProof,
    policy: &Policy,
    wallet: &[u8; 20],
    now: u64,
) -> Result<Credential, VerifyError> {
    // 1-2. Attestor signatures over the recomputed identifier.
    if proof.signatures.is_empty() {
        return Err(VerifyError::NoSignatures);
    }
    let signers = reclaim::recover_signers(proof)?;
    let trusted = policy.attestor_addresses();
    if !signers.iter().any(|s| trusted.contains(s)) {
        return Err(VerifyError::UntrustedAttestor);
    }

    // 3. The attested URL must be an allowed platform endpoint.
    let params: serde_json::Value = serde_json::from_str(&proof.claim_data.parameters)
        .map_err(|_| VerifyError::MalformedParameters)?;
    let url = params
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or(VerifyError::MalformedParameters)?;
    let (host, path) = split_url(url).ok_or(VerifyError::UnknownPlatform)?;
    let rule = policy
        .platforms
        .iter()
        .find(|r| {
            r.hosts.iter().any(|h| h.eq_ignore_ascii_case(&host))
                && (r.path_prefixes.is_empty()
                    || r.path_prefixes.iter().any(|p| path.starts_with(p.as_str())))
        })
        .ok_or(VerifyError::UnknownPlatform)?;

    // 4. Wallet binding + app domain separation.
    let ctx: serde_json::Value = if proof.claim_data.context.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&proof.claim_data.context).map_err(|_| VerifyError::MalformedContext)?
    };
    let ctx_addr = ctx
        .get("contextAddress")
        .and_then(|v| v.as_str())
        .ok_or(VerifyError::WalletMismatch)?;
    if reclaim::parse_address(ctx_addr).map_err(|_| VerifyError::WalletMismatch)? != *wallet {
        return Err(VerifyError::WalletMismatch);
    }
    let ctx_msg = ctx.get("contextMessage").and_then(|v| v.as_str()).unwrap_or("");
    if !ctx_msg.starts_with(&policy.context_tag) {
        return Err(VerifyError::WrongContextTag);
    }

    // 5. Freshness (allow 5 minutes of clock skew into the future).
    let ts = proof.claim_data.timestamp_s;
    if ts > now.saturating_add(300) || now.saturating_sub(ts) > policy.max_age_secs {
        return Err(VerifyError::Stale);
    }

    // 6. Fields.
    let fields = extracted_params(&ctx, &params);
    let title = first_of(&fields, &rule.title_keys)
        .filter(|t| !t.trim().is_empty())
        .ok_or(VerifyError::MissingField("title"))?
        .clone();
    let completion_pct = if rule.completion_keys.is_empty() {
        100
    } else {
        let raw = first_of(&fields, &rule.completion_keys)
            .ok_or(VerifyError::MissingField("completion"))?;
        parse_completion(raw, rule.completion_is_fraction)
            .ok_or(VerifyError::MissingField("completion"))?
    };
    let cert_id = first_of(&fields, &rule.cert_id_keys).cloned();

    Ok(Credential {
        platform: rule.name.clone(),
        title,
        completion_pct,
        cert_id,
        attested_at: ts,
    })
}

/// Verify every proof and evaluate the claim. Fails closed: one invalid proof
/// aborts the whole evaluation and reports its index.
pub fn evaluate(input: &ProgramInput) -> Result<Evaluation, (usize, VerifyError)> {
    let wallet = reclaim::parse_address(&input.wallet).map_err(|e| (usize::MAX, e))?;
    let policy = &input.policy;

    let mut seen = std::collections::BTreeSet::new();
    let mut counted_keys: Vec<[u8; 32]> = Vec::new();
    let mut credentials = Vec::new();

    for (i, proof) in input.proofs.iter().enumerate() {
        let cred = verify_proof(proof, policy, &wallet, input.evaluated_at).map_err(|e| (i, e))?;
        let key = cred.dedup_key();
        let verdict = if !seen.insert(key) {
            Verdict::Duplicate
        } else if cred.completion_pct < policy.min_completion_pct {
            Verdict::Incomplete
        } else if !policy.industry.matches(&cred.title) {
            Verdict::OtherIndustry
        } else {
            counted_keys.push(key);
            Verdict::Counted
        };
        credentials.push((cred, verdict));
    }

    counted_keys.sort();
    let mut h = Sha256::new();
    for k in &counted_keys {
        h.update(k);
    }
    let commitment: [u8; 32] = h.finalize().into();
    let qualified = counted_keys.len() as u32;

    Ok(Evaluation {
        public: PublicValuesStruct {
            wallet: Address::from(wallet),
            policyHash: FixedBytes::from(policy.hash()),
            industry: policy.industry.id.clone(),
            minCount: policy.min_count,
            qualifiedCount: qualified,
            passed: qualified >= policy.min_count,
            certSetCommitment: FixedBytes::from(commitment),
            evaluatedAt: input.evaluated_at,
        },
        credentials,
    })
}

/// ABI-encode public values (what the guest commits and Solidity decodes).
pub fn encode_public(pv: &PublicValuesStruct) -> Vec<u8> {
    pv.abi_encode()
}

/// Decode committed public values.
pub fn decode_public(bytes: &[u8]) -> Result<PublicValuesStruct, alloy_sol_types::Error> {
    PublicValuesStruct::abi_decode(bytes)
}
