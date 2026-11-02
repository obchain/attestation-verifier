# contracts — on-chain ExpertBadge

`ExpertBadge.claim(publicValues, proof)` verifies an SP1 proof of the `program/` guest through
Succinct's `SP1VerifierGateway`, decodes `PublicValuesStruct` (same field order as
`crates/core/src/lib.rs`), and records a non-transferable badge per (wallet, industry).

Checks, in order: proof valid for `programVKey` → `wallet == msg.sender` → `policyHash` equals the
deployed one → `passed` → `evaluatedAt` within `maxAge` and not in the future → `evaluatedAt`
strictly newer than the stored badge (blocks replay, allows refresh).

Zero dependencies: `ISP1Verifier` is copied from sp1-contracts v6.1.1, tests use raw cheatcodes.

```bash
cd contracts && forge test
```

`test_DecodesRustPublicValues` pins the bytes `encode_public` produces for
`fixtures/input.test.json`. If `PublicValuesStruct` changes in Rust, regenerate that hex and update
the struct in `src/ExpertBadge.sol`.

## Deploying to a testnet

Deployed on Sepolia (chain 11155111), one per policy: crypto `0x18e3324fe3adec58f1805a5c27d47a8ea715f995`, AI & ML `0xd72de5801d4b448d380eb1e59a696134427e70e5`, maxAge 30 days; `hasBadge` lapses after that until refreshed. Details in `fixtures/live/onchain.json`.

1. Produce an on-chain proof (needs the SP1 toolchain and the Groth16 artifacts/Docker or the
   prover network):

   ```bash
   cargo run --release --bin zkcert -- prove --input <input.json> --system groth16 --out proof.bin
   ```

   It prints `program vkey`, `onchain proof 0x…`, `public values 0x…`, and the decoded public values
   (incl. `policyHash`).

2. Pick the gateway. Groth16 gateway on Sepolia, from
   `sp1-contracts/contracts/deployments/11155111.json`: `0x397A5f7f3dBd538f23DE225B51f532c34448dA9B`.
   Check that file for other chains, and that the gateway routes the verifier version matching the
   SP1 SDK used to prove (first 4 bytes of the proof select the verifier).

3. Deploy with a keystore account (never pass a raw private key on the command line):

   ```bash
   cast wallet import deployer --interactive
   forge create src/ExpertBadge.sol:ExpertBadge --rpc-url $SEPOLIA_RPC_URL --account deployer --broadcast \
     --constructor-args 0x397A5f7f3dBd538f23DE225B51f532c34448dA9B <program-vkey> <policy-hash> 604800
   ```

   `policy-hash` must be the hash of the real `policy.json` (with trusted Reclaim attestors filled in),
   not the test policy.

4. Claim from the wallet the proofs are bound to (`contextAddress`):

   ```bash
   cast send <badge-address> "claim(bytes,bytes)" <public-values-hex> <onchain-proof-hex> \
     --rpc-url $SEPOLIA_RPC_URL --account <wallet-account>
   cast call <badge-address> "hasBadge(address,string)(bool)" <wallet> crypto --rpc-url $SEPOLIA_RPC_URL
   ```

## Design notes

- **`msg.sender` must be the bound wallet.** The badge asserts something about that wallet, so it
  consents by sending the tx. Cost: no gasless relaying; if wanted later, add an EIP-712 signature
  from the wallet instead of the sender check.
- **One policy per contract.** Policies are per industry and the policy hash binds the industry,
  attestors and thresholds, so one deployment = one industry. Multi-industry would need an
  allow-list of policy hashes (and an owner), skipped to stay minimal.
- **`maxAge` bounds `evaluatedAt`.** The host picks `evaluatedAt`; the guest checks attestation
  freshness relative to it, so without an on-chain bound an old timestamp would make stale
  attestations count.
