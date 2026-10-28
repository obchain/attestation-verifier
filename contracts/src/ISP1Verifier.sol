// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// Copied verbatim from succinctlabs/sp1-contracts v6.1.1 (contracts/src/ISP1Verifier.sol, MIT)
/// so this package has no dependencies. Deployments point at Succinct's SP1VerifierGateway.
interface ISP1Verifier {
    /// @notice Verifies a proof with given public values and vkey.
    /// @dev It is expected that the first 4 bytes of proofBytes must match the first 4 bytes of
    /// target verifier's VERIFIER_HASH.
    /// @param programVKey The verification key for the RISC-V program.
    /// @param publicValues The public values encoded as bytes.
    /// @param proofBytes The proof of the program execution the SP1 zkVM encoded as bytes.
    function verifyProof(bytes32 programVKey, bytes calldata publicValues, bytes calldata proofBytes) external view;
}
