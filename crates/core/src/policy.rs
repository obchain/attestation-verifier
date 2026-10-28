//! The rules a credential set is judged against. The policy is a public input:
//! its hash is committed in the proof's public values, so a verifier knows
//! exactly which attestors, endpoints and thresholds were applied.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    pub version: u32,
    /// Every proof's `contextMessage` must start with this tag. Domain-separates
    /// our proofs from proofs some other app requested for the same wallet.
    pub context_tag: String,
    /// Trusted Reclaim attestor addresses (from https://api.reclaimprotocol.org/api/attestors).
    pub attestors: Vec<String>,
    /// Platforms we accept credentials from, and how to read their fields.
    pub platforms: Vec<PlatformRule>,
    /// The Phi industry this proof is about.
    pub industry: Industry,
    /// Minimum number of distinct qualifying credentials.
    pub min_count: u32,
    /// Minimum course completion, in percent.
    pub min_completion_pct: u32,
    /// Maximum age of an attestation at evaluation time.
    pub max_age_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlatformRule {
    /// "udemy" | "coursera"
    pub name: String,
    /// Allowed hosts for the attested URL, e.g. "www.udemy.com".
    pub hosts: Vec<String>,
    /// Allowed URL path prefixes (pins the exact API endpoint). Empty = any path.
    pub path_prefixes: Vec<String>,
    /// Extracted-parameter names that hold the course / certificate title.
    pub title_keys: Vec<String>,
    /// Extracted-parameter names for completion. Empty means the endpoint only
    /// returns earned certificates, so completion is implied (100%).
    pub completion_keys: Vec<String>,
    /// True if completion is reported as a 0..1 fraction rather than 0..100.
    #[serde(default)]
    pub completion_is_fraction: bool,
    /// Extracted-parameter names for a certificate / credential ID (used for dedup).
    pub cert_id_keys: Vec<String>,
    /// Allowed Reclaim provider hashes (SDK `hashProofClaimParams`, `0x` hex). Pins the
    /// exact request and which response fields are extracted. Empty = reject all.
    #[serde(default)]
    pub provider_hashes: Vec<String>,
}

impl PlatformRule {
    pub fn provider_hash_bytes(&self) -> Vec<[u8; 32]> {
        self.provider_hashes
            .iter()
            .filter_map(|h| {
                let b = hex::decode(h.trim().trim_start_matches("0x")).ok()?;
                b.try_into().ok()
            })
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Industry {
    /// Phi industry id, e.g. "crypto", "ai-ml", "cybersecurity".
    pub id: String,
    /// Lowercase keywords matched against titles on word boundaries.
    /// A trailing `*` makes it a prefix match (e.g. "zk*" matches "zksync").
    pub keywords: Vec<String>,
}

impl Policy {
    /// sha256 of the policy's JSON serialization (struct field order is fixed).
    pub fn hash(&self) -> [u8; 32] {
        let bytes = serde_json::to_vec(self).expect("policy serializes");
        Sha256::digest(bytes).into()
    }

    pub fn attestor_addresses(&self) -> Vec<[u8; 20]> {
        self.attestors
            .iter()
            .filter_map(|a| crate::reclaim::parse_address(a).ok())
            .collect()
    }
}

impl Industry {
    pub fn matches(&self, title: &str) -> bool {
        let t = title.to_lowercase();
        self.keywords.iter().any(|k| keyword_in(&t, &k.to_lowercase()))
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// Word-boundary keyword search, so "defi" does not match "definitely".
fn keyword_in(text: &str, kw: &str) -> bool {
    let (kw, prefix) = match kw.strip_suffix('*') {
        Some(k) => (k, true),
        None => (kw, false),
    };
    if kw.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = text[start..].find(kw) {
        let i = start + pos;
        let before_ok = text[..i].chars().next_back().map_or(true, |c| !is_word(c));
        let after = &text[i + kw.len()..];
        let after_ok = prefix || after.chars().next().map_or(true, |c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
        start = i + kw.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_boundaries() {
        assert!(keyword_in("intro to defi lending", "defi"));
        assert!(!keyword_in("definitely not crypto", "defi"));
        assert!(keyword_in("zksync deep dive", "zk*"));
        assert!(!keyword_in("bazk tools", "zk*"));
        assert!(keyword_in("smart contract security", "smart contract"));
        assert!(keyword_in("ethereum & solidity", "solidity"));
    }
}
