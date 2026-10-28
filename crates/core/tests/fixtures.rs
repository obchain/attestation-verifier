//! End-to-end checks of the verification logic against proofs produced with the
//! official Reclaim SDK signing code (see prover/src/fixtures.ts).

use eulerphi_core::{
    decode_public, encode_public, evaluate, reclaim, verify_proof, ProgramInput, ReclaimProof,
    Verdict, VerifyError,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");

fn input() -> ProgramInput {
    let raw = std::fs::read_to_string(format!("{FIXTURES}/input.test.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn negative(name: &str) -> ReclaimProof {
    let raw = std::fs::read_to_string(format!("{FIXTURES}/negative/{name}.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn wallet(i: &ProgramInput) -> [u8; 20] {
    reclaim::parse_address(&i.wallet).unwrap()
}

#[test]
fn identifier_matches_sdk() {
    // The SDK wrote its own identifier into each fixture; we must recompute the same.
    for p in input().proofs {
        let ours = format!("0x{}", hex::encode(reclaim::claim_identifier(&p.claim_data).unwrap()));
        assert_eq!(ours, p.claim_data.identifier);
    }
}

#[test]
fn recovers_sdk_signed_attestor() {
    let i = input();
    let trusted = i.policy.attestor_addresses();
    for p in &i.proofs {
        let signers = reclaim::recover_signers(p).unwrap();
        assert_eq!(signers, trusted, "signer must be the test attestor");
    }
}

#[test]
fn happy_path_counts_three_distinct_crypto_credentials() {
    let i = input();
    let e = evaluate(&i).unwrap();
    let verdicts: Vec<_> = e.credentials.iter().map(|(c, v)| (c.title.as_str(), *v)).collect();
    assert_eq!(
        verdicts,
        vec![
            ("Ethereum and Solidity: The Complete Developer's Guide", Verdict::Counted),
            ("Blockchain Basics", Verdict::Counted),
            ("Zero-Knowledge Proofs in Rust", Verdict::Counted),
            ("Web3 dApp Bootcamp", Verdict::Incomplete),
            ("Machine Learning Specialization", Verdict::OtherIndustry),
            ("Ethereum and Solidity: The Complete Developer's Guide", Verdict::Duplicate),
        ]
    );
    assert_eq!(e.public.qualifiedCount, 3);
    assert!(e.public.passed);
    assert_eq!(e.public.industry, "crypto");
    assert_eq!(e.public.wallet.0 .0, wallet(&i));
}

#[test]
fn public_values_roundtrip_abi() {
    let e = evaluate(&input()).unwrap();
    let bytes = encode_public(&e.public);
    assert_eq!(decode_public(&bytes).unwrap(), e.public);
}

#[test]
fn commitment_is_order_independent() {
    let mut i = input();
    let a = evaluate(&i).unwrap().public.certSetCommitment;
    i.proofs.reverse();
    let b = evaluate(&i).unwrap().public.certSetCommitment;
    assert_eq!(a, b);
}

#[test]
fn threshold_not_met_still_proves_but_fails() {
    let mut i = input();
    i.policy.min_count = 4;
    let e = evaluate(&i).unwrap();
    assert_eq!(e.public.qualifiedCount, 3);
    assert!(!e.public.passed);
}

#[test]
fn policy_change_changes_policy_hash() {
    let mut i = input();
    let a = evaluate(&i).unwrap().public.policyHash;
    i.policy.industry.keywords.push("python".into());
    let b = evaluate(&i).unwrap().public.policyHash;
    assert_ne!(a, b);
}

fn reject(p: &ReclaimProof) -> VerifyError {
    let i = input();
    verify_proof(p, &i.policy, &wallet(&i), i.evaluated_at).unwrap_err()
}

#[test]
fn rejects_proof_bound_to_another_wallet() {
    assert_eq!(reject(&negative("other-wallet")), VerifyError::WalletMismatch);
}

#[test]
fn rejects_untrusted_attestor() {
    assert_eq!(reject(&negative("rogue-attestor")), VerifyError::UntrustedAttestor);
}

#[test]
fn rejects_look_alike_host() {
    assert_eq!(reject(&negative("look-alike-host")), VerifyError::UnknownPlatform);
}

#[test]
fn rejects_non_pinned_endpoint() {
    assert_eq!(reject(&negative("wrong-endpoint")), VerifyError::UnknownPlatform);
}

#[test]
fn rejects_foreign_app_context() {
    assert_eq!(reject(&negative("foreign-app-tag")), VerifyError::WrongContextTag);
}

#[test]
fn rejects_stale_attestation() {
    assert_eq!(reject(&negative("stale")), VerifyError::Stale);
}

#[test]
fn provider_hash_matches_sdk() {
    // The test policy's provider hashes were computed by the SDK's own
    // `hashProofClaimParams`; we must compute the same from each proof.
    let i = input();
    for p in &i.proofs {
        let params: serde_json::Value = serde_json::from_str(&p.claim_data.parameters).unwrap();
        let ours = reclaim::provider_hashes(&params).unwrap();
        let pinned: Vec<[u8; 32]> =
            i.policy.platforms.iter().flat_map(|r| r.provider_hash_bytes()).collect();
        assert!(ours.iter().any(|h| pinned.contains(h)), "no SDK-pinned hash matches");
    }
}

#[test]
fn rejects_claimant_written_extraction_spec() {
    assert_eq!(reject(&negative("unpinned-provider")), VerifyError::UnpinnedProvider);
}

#[test]
fn ignores_claimant_param_values_as_fields() {
    assert_eq!(reject(&negative("param-values-title")), VerifyError::MissingField("title"));
}

#[test]
fn rejects_tag_that_only_shares_a_prefix() {
    assert_eq!(reject(&negative("tag-prefix")), VerifyError::WrongContextTag);
}

#[test]
fn empty_provider_pin_rejects_everything() {
    let mut i = input();
    for r in &mut i.policy.platforms {
        r.provider_hashes.clear();
    }
    let p = i.proofs[0].clone();
    assert_eq!(
        verify_proof(&p, &i.policy, &wallet(&i), i.evaluated_at).unwrap_err(),
        VerifyError::UnpinnedProvider
    );
}

#[test]
fn tampered_title_breaks_signature_binding() {
    // Editing the extracted course title changes the identifier, so the
    // signature recovers to a random address, not the attestor.
    let mut p = input().proofs[0].clone();
    p.claim_data.context = p.claim_data.context.replace("Ethereum", "Ethereun");
    assert_eq!(reject(&p), VerifyError::UntrustedAttestor);
}

#[test]
fn tampered_wallet_breaks_signature_binding() {
    let i = input();
    let mut p = i.proofs[0].clone();
    let lower = i.wallet.to_lowercase();
    p.claim_data.context = p
        .claim_data
        .context
        .replace(&i.wallet, "0x000000000000000000000000000000000000dEaD")
        .replace(&lower, "0x000000000000000000000000000000000000dead");
    assert_eq!(reject(&p), VerifyError::UntrustedAttestor);
}

#[test]
fn reordered_context_keys_still_verify() {
    // RFC 8785 canonicalization: key order in the context string must not matter.
    let i = input();
    let mut p = i.proofs[0].clone();
    let v: serde_json::Value = serde_json::from_str(&p.claim_data.context).unwrap();
    let obj = v.as_object().unwrap();
    let mut parts: Vec<String> =
        obj.iter().map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v)).collect();
    parts.reverse();
    p.claim_data.context = format!("{{ {} }}", parts.join(", "));
    assert!(verify_proof(&p, &i.policy, &wallet(&i), i.evaluated_at).is_ok());
}

#[test]
fn whole_set_fails_closed_on_one_bad_proof() {
    let mut i = input();
    i.proofs.push(negative("rogue-attestor"));
    let (idx, err) = evaluate(&i).unwrap_err();
    assert_eq!(idx, 6);
    assert_eq!(err, VerifyError::UntrustedAttestor);
}

#[test]
fn rejects_unsigned_and_malformed() {
    let mut p = input().proofs[0].clone();
    p.signatures.clear();
    assert_eq!(reject(&p), VerifyError::NoSignatures);
    p.signatures = vec!["0x1234".into()];
    assert_eq!(reject(&p), VerifyError::BadSignature);
}
