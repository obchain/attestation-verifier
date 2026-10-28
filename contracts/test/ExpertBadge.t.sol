// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "../src/ISP1Verifier.sol";
import {ExpertBadge, PublicValuesStruct} from "../src/ExpertBadge.sol";

/// The few forge cheatcodes we use (no forge-std).
interface Vm {
    function warp(uint256) external;
    function prank(address) external;
    function expectRevert(bytes4) external;
}

/// Accepts exactly `keccak256(abi.encode(vkey, publicValues))` as the proof, so tests also
/// check that ExpertBadge forwards the right vkey and public values.
contract MockVerifier is ISP1Verifier {
    error InvalidProof();

    function verifyProof(bytes32 vkey, bytes calldata publicValues, bytes calldata proof) external pure {
        if (keccak256(proof) != keccak256(fakeProof(vkey, publicValues))) revert InvalidProof();
    }
}

function fakeProof(bytes32 vkey, bytes memory publicValues) pure returns (bytes memory) {
    return abi.encodePacked(keccak256(abi.encode(vkey, publicValues)));
}

contract ExpertBadgeTest {
    Vm constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    /// `encode_public(evaluate(fixtures/input.test.json).public)` from crates/core.
    bytes constant RUST_PV = hex"0000000000000000000000000000000000000000000000000000000000000020"
        hex"0000000000000000000000005aae27dacaf8b6b7a6eea2406bf356f105cae7c5"
        hex"b0c3e1edc118dae3254aae911a2ce963bdac745c366ec31bfa935cba8d4be5bb"
        hex"0000000000000000000000000000000000000000000000000000000000000100"
        hex"0000000000000000000000000000000000000000000000000000000000000002"
        hex"0000000000000000000000000000000000000000000000000000000000000003"
        hex"0000000000000000000000000000000000000000000000000000000000000001"
        hex"fd1c7bf989a31d7d22275618efd6027da28c8b7d30b3a5aed25015056687897d"
        hex"000000000000000000000000000000000000000000000000000000006ae19d70"
        hex"0000000000000000000000000000000000000000000000000000000000000006"
        hex"63727970746f0000000000000000000000000000000000000000000000000000";

    address constant WALLET = 0x5Aae27DacAf8b6b7A6eEA2406bf356F105cae7C5;
    bytes32 constant POLICY = 0xb0c3e1edc118dae3254aae911a2ce963bdac745c366ec31bfa935cba8d4be5bb;
    bytes32 constant COMMITMENT = 0xfd1c7bf989a31d7d22275618efd6027da28c8b7d30b3a5aed25015056687897d;
    uint64 constant EVALUATED_AT = 1793170800;
    bytes32 constant VKEY = keccak256("program vkey");
    uint64 constant MAX_AGE = 7 days;

    MockVerifier mock;
    ExpertBadge badge;

    function setUp() public {
        mock = new MockVerifier();
        badge = new ExpertBadge(mock, VKEY, POLICY, MAX_AGE);
        vm.warp(EVALUATED_AT + 1 hours);
    }

    function rustPv() internal pure returns (PublicValuesStruct memory) {
        return abi.decode(RUST_PV, (PublicValuesStruct));
    }

    function claimAs(address sender, PublicValuesStruct memory pv) internal {
        bytes memory raw = abi.encode(pv);
        vm.prank(sender);
        badge.claim(raw, fakeProof(VKEY, raw));
    }

    function test_DecodesRustPublicValues() public pure {
        PublicValuesStruct memory pv = rustPv();
        require(pv.wallet == WALLET, "wallet");
        require(pv.policyHash == POLICY, "policyHash");
        require(keccak256(bytes(pv.industry)) == keccak256("crypto"), "industry");
        require(pv.minCount == 2, "minCount");
        require(pv.qualifiedCount == 3, "qualifiedCount");
        require(pv.passed, "passed");
        require(pv.certSetCommitment == COMMITMENT, "certSetCommitment");
        require(pv.evaluatedAt == EVALUATED_AT, "evaluatedAt");
        // Byte-for-byte: Solidity's encoding of the decoded struct equals Rust's bytes.
        require(keccak256(abi.encode(pv)) == keccak256(RUST_PV), "re-encode");
    }

    function test_ClaimWithRustBytes() public {
        vm.prank(WALLET);
        badge.claim(RUST_PV, fakeProof(VKEY, RUST_PV));

        require(badge.hasBadge(WALLET, "crypto"), "badge");
        require(!badge.hasBadge(WALLET, "ai"), "other industry");
        (uint32 count, uint64 at, bytes32 commitment) = badge.badges(WALLET, keccak256("crypto"));
        require(count == 3 && at == EVALUATED_AT && commitment == COMMITMENT, "stored");
    }

    function test_RevertInvalidProof() public {
        vm.prank(WALLET);
        vm.expectRevert(MockVerifier.InvalidProof.selector);
        badge.claim(RUST_PV, fakeProof(keccak256("other vkey"), RUST_PV));
    }

    function test_RevertWrongWallet() public {
        vm.expectRevert(ExpertBadge.WrongWallet.selector);
        claimAs(address(0xBEEF), rustPv());
    }

    function test_RevertWrongPolicyHash() public {
        PublicValuesStruct memory pv = rustPv();
        pv.policyHash = keccak256("another policy");
        vm.expectRevert(ExpertBadge.WrongPolicy.selector);
        claimAs(WALLET, pv);
    }

    function test_RevertNotPassed() public {
        PublicValuesStruct memory pv = rustPv();
        pv.minCount = 4;
        pv.passed = false;
        vm.expectRevert(ExpertBadge.NotPassed.selector);
        claimAs(WALLET, pv);
    }

    function test_RevertReplay() public {
        claimAs(WALLET, rustPv());
        vm.expectRevert(ExpertBadge.NotNewer.selector);
        claimAs(WALLET, rustPv());
    }

    function test_RefreshWithNewerProof() public {
        claimAs(WALLET, rustPv());
        PublicValuesStruct memory pv = rustPv();
        pv.evaluatedAt = EVALUATED_AT + 30 minutes;
        pv.qualifiedCount = 4;
        claimAs(WALLET, pv);
        (uint32 count, uint64 at,) = badge.badges(WALLET, keccak256("crypto"));
        require(count == 4 && at == EVALUATED_AT + 30 minutes, "refreshed");
    }

    function test_RevertStale() public {
        vm.warp(EVALUATED_AT + MAX_AGE + 1);
        vm.expectRevert(ExpertBadge.NotFresh.selector);
        claimAs(WALLET, rustPv());
    }

    function test_RevertFutureEvaluation() public {
        vm.warp(EVALUATED_AT - 1);
        vm.expectRevert(ExpertBadge.NotFresh.selector);
        claimAs(WALLET, rustPv());
    }
}
