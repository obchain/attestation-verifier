//! Reclaim Protocol proof format and attestor-signature verification.
//!
//! This mirrors `@reclaimprotocol/js-sdk` v5.8.x exactly:
//!
//! * `getIdentifierFromClaimInfo`:
//!   `identifier = keccak256(provider + "\n" + parameters + "\n" + canonical(context))`,
//!   lowercase `0x` hex, where `canonical` is RFC 8785 JSON (sorted keys, no whitespace).
//! * `createSignDataForClaim`:
//!   `identifier + "\n" + owner.lower() + "\n" + timestampS + "\n" + epoch`
//! * Attestors sign that string as an Ethereum personal message (EIP-191) and
//!   `ethers.verifyMessage` recovers the attestor address.
//!
//! The identifier is always RECOMPUTED here; the `identifier` field inside the
//! proof is never trusted.

use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Keccak};

use crate::error::VerifyError;

/// A proof exactly as the Reclaim SDK returns it (unknown fields ignored).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReclaimProof {
    #[serde(default)]
    pub identifier: String,
    pub claim_data: ClaimData,
    /// 65-byte `r || s || v` signatures, `0x` hex.
    pub signatures: Vec<String>,
    #[serde(default)]
    pub witnesses: Vec<Witness>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaimData {
    /// Provider kind, normally `"http"`.
    pub provider: String,
    /// JSON string: url, method, response matches/redactions, paramValues.
    pub parameters: String,
    /// Address of the key that requested the claim (the Reclaim app / session key).
    pub owner: String,
    pub timestamp_s: u64,
    /// JSON string: contextAddress, contextMessage, extractedParameters, providerHash, ...
    pub context: String,
    #[serde(default)]
    pub identifier: String,
    pub epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Witness {
    pub id: String,
    #[serde(default)]
    pub url: String,
}

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut k = Keccak::v256();
    k.update(data);
    k.finalize(&mut out);
    out
}

/// RFC 8785 canonical form of a JSON string, as produced by the `canonicalize`
/// npm package. serde_json's default `Map` is a BTreeMap, so object keys come
/// out sorted and the output has no whitespace.
///
/// Limitation: JCS sorts keys by UTF-16 code units while BTreeMap sorts by
/// UTF-8 bytes; they agree for ASCII keys, which is all Reclaim uses. Non-integer
/// numbers are not expected in contexts (all values are strings).
pub fn canonical_json(raw: &str) -> Result<String, VerifyError> {
    if raw.is_empty() {
        return Ok(String::new());
    }
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| VerifyError::MalformedContext)?;
    serde_json::to_string(&v).map_err(|_| VerifyError::MalformedContext)
}

/// `keccak256(provider \n parameters \n canonical(context))`
pub fn claim_identifier(claim: &ClaimData) -> Result<[u8; 32], VerifyError> {
    let ctx = canonical_json(&claim.context)?;
    let s = format!("{}\n{}\n{}", claim.provider, claim.parameters, ctx);
    Ok(keccak256(s.as_bytes()))
}

/// The exact string the attestor signs.
pub fn sign_data(claim: &ClaimData) -> Result<String, VerifyError> {
    let id = claim_identifier(claim)?;
    Ok(format!(
        "0x{}\n{}\n{}\n{}",
        hex::encode(id),
        claim.owner.to_lowercase(),
        claim.timestamp_s,
        claim.epoch
    ))
}

/// EIP-191 personal-message hash.
pub fn eip191_hash(msg: &[u8]) -> [u8; 32] {
    let mut buf = format!("\x19Ethereum Signed Message:\n{}", msg.len()).into_bytes();
    buf.extend_from_slice(msg);
    keccak256(&buf)
}

pub fn parse_address(s: &str) -> Result<[u8; 20], VerifyError> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(h).map_err(|_| VerifyError::BadAddress)?;
    bytes.try_into().map_err(|_| VerifyError::BadAddress)
}

/// Recover the Ethereum address that produced a 65-byte personal-sign signature.
pub fn recover_signer(msg: &str, sig_hex: &str) -> Result<[u8; 20], VerifyError> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    let h = sig_hex.strip_prefix("0x").unwrap_or(sig_hex);
    let sig = hex::decode(h).map_err(|_| VerifyError::BadSignature)?;
    if sig.len() != 65 {
        return Err(VerifyError::BadSignature);
    }
    let v = match sig[64] {
        27 | 28 => sig[64] - 27,
        0 | 1 => sig[64],
        _ => return Err(VerifyError::BadSignature),
    };
    let signature = Signature::from_slice(&sig[..64]).map_err(|_| VerifyError::BadSignature)?;
    // Reject high-s (malleable) signatures, like ethers does.
    if signature.normalize_s().is_some() {
        return Err(VerifyError::BadSignature);
    }
    let rid = RecoveryId::from_byte(v).ok_or(VerifyError::BadSignature)?;
    let digest = eip191_hash(msg.as_bytes());
    let vk = VerifyingKey::recover_from_prehash(&digest, &signature, rid)
        .map_err(|_| VerifyError::BadSignature)?;
    let point = vk.to_encoded_point(false);
    let hash = keccak256(&point.as_bytes()[1..]);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    Ok(addr)
}

/// Recover every signer of a proof's claim.
pub fn recover_signers(proof: &ReclaimProof) -> Result<Vec<[u8; 20]>, VerifyError> {
    let msg = sign_data(&proof.claim_data)?;
    proof
        .signatures
        .iter()
        .map(|s| recover_signer(&msg, s))
        .collect()
}

/// Most `responseMatches` a claim may carry. The SDK hashes every subset of the
/// optional matches (2^n), so this bounds the work done inside the zkVM.
const MAX_RESPONSE_MATCHES: usize = 12;

/// Provider hashes of a claim's HTTP parameters, mirroring the SDK's
/// `hashProofClaimParams` / `getProviderParamsAsCanonicalizedString`: keccak256
/// of the canonical JSON of `{url, method, body, responseMatches, responseRedactions}`,
/// one hash per valid combination of optional matches. `paramValues` is not part
/// of the hash, exactly as in the SDK.
pub fn provider_hashes(params: &serde_json::Value) -> Result<Vec<[u8; 32]>, VerifyError> {
    use serde_json::{json, Map, Value};

    let empty = Vec::new();
    let matches = params
        .get("responseMatches")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let redactions = params
        .get("responseRedactions")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    if matches.len() > MAX_RESPONSE_MATCHES {
        return Err(VerifyError::MalformedParameters);
    }
    let or = |v: Option<&Value>, d: &str| match v {
        Some(Value::Null) | None => Value::String(d.to_string()),
        Some(x) => x.clone(),
    };
    // JS `x || undefined`: keep only truthy values.
    let truthy = |v: Option<&Value>| match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => None,
        Some(x) => Some(x.clone()),
    };
    let canonical = |m: Vec<Value>, r: Vec<Value>| -> Result<[u8; 32], VerifyError> {
        let filtered = json!({
            "url": or(params.get("url"), ""),
            "method": or(params.get("method"), "GET"),
            "body": or(params.get("body"), ""),
            "responseMatches": m,
            "responseRedactions": r,
        });
        let s = serde_json::to_string(&filtered).map_err(|_| VerifyError::MalformedParameters)?;
        Ok(keccak256(s.as_bytes()))
    };

    let mut out = Vec::new();
    for mask in 1u32..(1u32 << matches.len()) {
        let (mut m, mut r) = (Vec::new(), Vec::new());
        let mut valid = true;
        for j in 0..matches.len() {
            let mj = &matches[j];
            if mask & (1 << j) != 0 {
                let mut o = Map::new();
                o.insert("value".into(), or(mj.get("value"), ""));
                o.insert("type".into(), or(mj.get("type"), "contains"));
                if let Some(inv) = truthy(mj.get("invert")) {
                    o.insert("invert".into(), inv);
                }
                m.push(Value::Object(o));
                if let Some(rj) = redactions.get(j).filter(|v| !v.is_null()) {
                    let mut o = Map::new();
                    o.insert("xPath".into(), or(rj.get("xPath"), ""));
                    o.insert("jsonPath".into(), or(rj.get("jsonPath"), ""));
                    o.insert("regex".into(), or(rj.get("regex"), ""));
                    if let Some(h) = truthy(rj.get("hash")) {
                        o.insert("hash".into(), h);
                    }
                    r.push(Value::Object(o));
                }
            } else if truthy(mj.get("isOptional")).is_none() {
                valid = false;
                break;
            }
        }
        if valid {
            out.push(canonical(m, r)?);
        }
    }
    if out.is_empty() {
        out.push(canonical(Vec::new(), Vec::new())?);
    }
    Ok(out)
}
