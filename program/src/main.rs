//! SP1 guest: "this wallet is a verified expert in industry X".
//!
//! Private inputs : Reclaim zkTLS proofs (course titles, certificate ids, attestor sigs).
//! Public outputs : wallet, policy hash, industry, min/qualified count, pass flag,
//!                  commitment to the credential set, evaluation time.
//!
//! The program aborts (no proof can be produced) if ANY proof is forged, comes
//! from an untrusted attestor or a non-allowed endpoint, is bound to another
//! wallet or app, or is stale.

#![no_main]
sp1_zkvm::entrypoint!(main);

use eulerphi_core::{encode_public, evaluate, ProgramInput};

pub fn main() {
    let input = sp1_zkvm::io::read::<ProgramInput>();

    let evaluation = match evaluate(&input) {
        Ok(e) => e,
        Err((i, err)) => panic!("proof #{i} rejected: {err}"),
    };

    sp1_zkvm::io::commit_slice(&encode_public(&evaluation.public));
}
