// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "./ISP1Verifier.sol";

/// Must match `PublicValuesStruct` in crates/core/src/lib.rs field for field.
struct PublicValuesStruct {
    address wallet;
    bytes32 policyHash;
    string industry;
    uint32 minCount;
    uint32 qualifiedCount;
    bool passed;
    bytes32 certSetCommitment;
    uint64 evaluatedAt;
}

/// Non-transferable "verified expert in industry X" badge, granted from an SP1 proof of the
/// eulerphi-zkcert program. One badge per (wallet, industry); a newer proof refreshes it.
contract ExpertBadge {
    struct Badge {
        uint32 qualifiedCount;
        uint64 evaluatedAt;
        bytes32 certSetCommitment;
    }

    ISP1Verifier public immutable verifier;
    bytes32 public immutable programVKey;
    /// sha256 of the policy the proof must have been evaluated under (binds industry, attestors,
    /// thresholds). Policies are per industry, so this contract issues badges for one industry.
    bytes32 public immutable policyHash;
    /// Max age of `evaluatedAt`, at claim time and for `hasBadge`. The host chooses `evaluatedAt`,
    /// and attestation freshness inside the program is relative to it, so the chain must bound it.
    uint64 public immutable maxAge;

    mapping(address wallet => mapping(bytes32 industryKey => Badge)) public badges;

    event BadgeClaimed(
        address indexed wallet,
        bytes32 indexed industryKey,
        string industry,
        uint32 qualifiedCount,
        bytes32 certSetCommitment,
        uint64 evaluatedAt
    );

    error WrongWallet();
    error WrongPolicy();
    error NotPassed();
    error NotFresh();
    error NotNewer();

    constructor(ISP1Verifier verifier_, bytes32 programVKey_, bytes32 policyHash_, uint64 maxAge_) {
        verifier = verifier_;
        programVKey = programVKey_;
        policyHash = policyHash_;
        maxAge = maxAge_;
    }

    function claim(bytes calldata publicValues, bytes calldata proof) external {
        verifier.verifyProof(programVKey, publicValues, proof);
        PublicValuesStruct memory pv = abi.decode(publicValues, (PublicValuesStruct));

        // Only the bound wallet may claim: the badge is its statement, not a relayer's.
        if (pv.wallet != msg.sender) revert WrongWallet();
        if (pv.policyHash != policyHash) revert WrongPolicy();
        if (!pv.passed) revert NotPassed();
        if (pv.evaluatedAt > block.timestamp || block.timestamp - pv.evaluatedAt > maxAge) {
            revert NotFresh();
        }

        bytes32 industryKey = keccak256(bytes(pv.industry));
        Badge storage b = badges[pv.wallet][industryKey];
        // Strictly newer evaluation: rejects replays of the same proof and rollbacks.
        if (pv.evaluatedAt <= b.evaluatedAt) revert NotNewer();

        b.qualifiedCount = pv.qualifiedCount;
        b.evaluatedAt = pv.evaluatedAt;
        b.certSetCommitment = pv.certSetCommitment;
        emit BadgeClaimed(pv.wallet, industryKey, pv.industry, pv.qualifiedCount, pv.certSetCommitment, pv.evaluatedAt);
    }

    /// A badge lapses `maxAge` after its evaluation, until a newer proof refreshes it.
    function hasBadge(address wallet, string calldata industry) external view returns (bool) {
        uint64 evaluatedAt = badges[wallet][keccak256(bytes(industry))].evaluatedAt;
        return evaluatedAt != 0 && block.timestamp - evaluatedAt <= maxAge;
    }
}
