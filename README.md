# eulerphi-zkcert

**Verified-expert proofs for Phi, from zkTLS-attested Udemy and Coursera course completions, proven in SP1.**
Runs live end to end on both platforms: a real account → our own zkTLS attestor → SP1 proof → a badge on Sepolia
([contract](https://sepolia.etherscan.io/address/0x18e3324fe3adec58f1805a5c27d47a8ea715f995), [claim tx](https://sepolia.etherscan.io/tx/0xb84069bc517d0580e69ec7868ce25f4f8d7100a1b25f5c61298f233d29c8ab6b)).

> *"Wallet `0x5Aae…e7C5` holds at least 2 distinct, fully completed courses in the **Crypto**
> industry, attested from udemy.com."*
> The verifier learns that sentence. It does not learn which courses, the account, or the email.

## Why this matters for EulerPhi

Phi is a trust layer of *verified experts* organized by industry, using ZK proofs instead of data
collection. Its "Education" proof (degrees and professional certifications) needs exactly this:

| Problem | How this repo solves it |
|---|---|
| Certificate links are public; anyone can claim someone else's | **zkTLS**: the data is attested straight from the platform's servers inside the *prover's own logged-in session* |
| Proving a credential usually means revealing it | Only selected fields are revealed to the prover's machine; SP1 reveals **nothing** but the verdict |
| Proofs can be replayed by another user or app | Each zkTLS proof is **bound to the wallet** and to this app's context tag, inside the signed claim |
| Phi needs "expert in industry X", not "has course Y" | SP1 program aggregates credentials, dedups, matches the **Phi industry**, applies thresholds |

## Architecture

```
Udemy ──TLS (via attestor)──▶ self-hosted attestor ──signed claim──▶ proofs/*.json
        prover/src/attest.ts    (Reclaim attestor-core,
        cookie stays private     ZK-checked redactions)
                                                               │
          policy.json (attestors, endpoints, industry, N) ─────┤
                                                               ▼
                         SP1 zkVM: program/ → crates/core::evaluate
                         ├─ recompute claim id, recover attestor sig (EIP-191)
                         ├─ trusted attestor? allowed host + pinned endpoint?
                         ├─ bound to this wallet + app tag? fresh?
                         ├─ extract title / completion / cert id
                         └─ dedup → industry match → count → commit
                                                               ▼
               public values: wallet, industry, qualified/min, passed,
                              policyHash, certSetCommitment, evaluatedAt
```

Full HLD, LLD and threat model: [docs/DESIGN.md](docs/DESIGN.md).

| Path | What |
|---|---|
| `crates/core` | All verification logic. Shared by zkVM guest and host, unit-tested natively |
| `program` | SP1 guest: read input → `evaluate` → commit ABI-encoded public values |
| `script` | `zkcert` CLI: `check`, `execute`, `prove`, `verify`, `vkey` |
| `prover` | TypeScript zkTLS side: `attest.ts` (live claims on our attestor), `request.ts` (Reclaim hosted flow), fixtures, SDK cross-check |
| `fixtures` | Test proofs signed via the Reclaim SDK with a throwaway test attestor; `fixtures/live/` holds a real Udemy claim |

## Quick start

**1. Tests (no SP1 toolchain needed)**

```bash
cargo test            # 31 tests: SDK compatibility, a real Udemy claim, forgery/replay/staleness/spec rejections
```

**2. Prove with fixtures**

```bash
curl -L https://sp1up.succinct.xyz | bash && sp1up --version 6.10.0   # matches sp1-sdk pin
brew install protobuf   # sp1-sdk's build script needs `protoc` (apt: protobuf-compiler)
cd script               # host builds on Rust 1.91.1 (rust-toolchain.toml); guest on `succinct`
cargo run --release -- check   --input ../fixtures/input.test.json   # native dry run
cargo run --release -- execute --input ../fixtures/input.test.json   # in zkVM, cycle count
cargo run --release -- prove   --input ../fixtures/input.test.json --system compressed
cargo run --release -- verify  --proof proof.bin
```

`check` output on the fixtures:

```
COUNTED  [udemy] Ethereum and Solidity: The Complete Developer's Guide (100%)
COUNTED  [coursera] Blockchain Basics (100%)
COUNTED  [udemy] Zero-Knowledge Proofs in Rust (100%)
partial  [udemy] Web3 dApp Bootcamp (45%)
other    [coursera] Machine Learning Specialization (100%)
dup      [udemy] Ethereum and Solidity: The Complete Developer's Guide (100%)
── public values (all a verifier learns) ──
industry          crypto
qualified / min   3 / 2
passed            true
```

**3. Live: your own Udemy account, your own attestor**

Run the attestor (Reclaim's open-source [attestor-core](https://github.com/reclaimprotocol/attestor-core),
tested at commit `dd43dc8`, Node ≥ 22):

```bash
git clone https://github.com/reclaimprotocol/attestor-core ../attestor-core && cd ../attestor-core
git checkout dd43dc8
npm ci --omit=dev --ignore-scripts && npm run download:zk-files
(umask 077; echo "PRIVATE_KEY=0x$(openssl rand -hex 32)" > .env.development)
node --experimental-strip-types src/scripts/start-server.ts   # ws://localhost:8001/ws; logs signerAddress
```

Put the logged `signerAddress` in `policy.json` `"attestors"` (ours, `0x3dafd19a…5485`, is already
there). Then, logged into Udemy in Chrome: DevTools → Network → any `subscribed-courses` request →
Copy as cURL, and `pbpaste > ~/udemy.curl && chmod 600 ~/udemy.curl`.

```bash
cd prover && npm install
npm run attest -- --wallet 0xYourWallet          # claim on your most recently accessed course; open
                                                 # your other completed course on Udemy and repeat
npm run attest -- --wallet 0xYourWallet --platform coursera [--start N]   # enrollment N, from ~/coursera.curl
npm run sdk-verify -- ../proofs/udemy-*.json     # Reclaim's SDK recomputes the same identifier
cd ../script
cargo run --release -- check  --proofs-dir ../proofs --policy ../policy.json --wallet 0xYourWallet
cargo run --release -- prove  --proofs-dir ../proofs --policy ../policy.json --wallet 0xYourWallet --blinding 0x…
cargo run --release -- verify --proof proof.bin
```

Real run (2026-10-31): claim in 3.4 s (5 ZK proofs for the redactions), SP1 compressed proof in
76 s, `proof is VALID`. With two completed courses (2026-11-02): `qualified 2 / 2`, `passed true`.

**4. On-chain (Sepolia, live)**

```bash
cargo run --release -- prove --proofs-dir ../proofs --policy ../policy.json --wallet 0xYourWallet --system groth16 --out onchain.bin
# prints `program vkey`, `onchain proof 0x…`, `public values 0x…`. Needs Docker; first run downloads the circuits.
cast send <badge> "claim(bytes,bytes)" <public-values> <onchain-proof> --rpc-url $SEPOLIA_RPC_URL --account <wallet>
```

Check the deployed badge yourself, no wallet needed:

```bash
cast call 0x18e3324fe3adec58f1805a5c27d47a8ea715f995 "hasBadge(address,string)(bool)" 0xDAcEa3F3ef21eC7A525591810FcD4615D5154431 crypto --rpc-url https://ethereum-sepolia-rpc.publicnode.com
```

| On Sepolia (2026-11-02) | |
|---|---|
| `ExpertBadge` | [`0x18e3324fe3adec58f1805a5c27d47a8ea715f995`](https://sepolia.etherscan.io/address/0x18e3324fe3adec58f1805a5c27d47a8ea715f995) ([deploy tx](https://sepolia.etherscan.io/tx/0x4f0d83c11cad8332d32b608ece8da776dff7445d93834b34288d538dbccbf183)) |
| Claim, verified by Succinct's gateway `0x397A…dA9B` (v6.1.0 Groth16) | [`0xb84069bc…`](https://sepolia.etherscan.io/tx/0xb84069bc517d0580e69ec7868ce25f4f8d7100a1b25f5c61298f233d29c8ab6b), 482,540 gas |
| Result | `hasBadge(0xDAcE…4431, "crypto") = true`, `qualifiedCount 2` (2 Udemy courses, `policy.json`) |
| Second badge, same program: AI & ML | [`0xd72de5801d4b448d380eb1e59a696134427e70e5`](https://sepolia.etherscan.io/address/0xd72de5801d4b448d380eb1e59a696134427e70e5) ([claim tx](https://sepolia.etherscan.io/tx/0x9e7c82e7c813c86d78b71e7529cb9b9c95d42f9e5ac57381810af9e44facfd4f)), `hasBadge(…, "ai-ml") = true`, `qualifiedCount 1` (1 Coursera course, `policy.ai-ml.json`) |
| Simulated with `eth_call` | Contract: other sender → `WrongWallet`, same proof again → `NotNewer`. Succinct's gateway directly: tampered public values → reverts |

Proofs and public values: [onchain.json](fixtures/live/onchain.json), [onchain-ai-ml.json](fixtures/live/onchain-ai-ml.json). The Groth16 proof is 356 bytes; the
first run took 41 min including the circuit download, later runs about 4.5 min.

Reclaim's hosted flow (`npm run request`) is kept: it works the same way once Reclaim's Udemy
provider is fixed (see below).

## Design choices

**Own attestor, Reclaim's protocol.** Reclaim's public "Udemy Course Name" provider is stale: it
waits for `subscribed-courses?fields[course]=…completion_ratio…title…`, but Udemy's My Learning page
now requests `fields[course]=archive_time,favorite_time,features,is_private,num_collections,tracking_id,url`,
so the hosted flow never fires (evidence in [docs/PROVIDERS.md](docs/PROVIDERS.md#0-update-2026-10-31-the-hosted-provider-is-stale)).
Reclaim's provider builder was also down (`GenerateSession UNKNOWN`). Instead of waiting, we run
Reclaim's open-source attestor ourselves and send our own request spec, which asks Udemy for exactly
`title,completion_ratio`. The claim format is unchanged, so the verifier didn't change a line: it checks
the protocol, not the vendor. `policy.json` decides which attestors to trust.

**Reclaim over TLSNotary.** Both are zkTLS. Reclaim's output is a secp256k1 attestor signature
over a hashed claim, which SP1 verifies cheaply with its keccak and secp256k1 precompiles, and it
already has Udemy providers. Verifying a TLSNotary presentation inside the zkVM would mean porting
its verifier crates; TLSNotary with a self-run notary is the natural next backend (see DESIGN §5).

**Our own verifier, not the SDK's.** The trust boundary is the zkVM, so `crates/core` re-implements
Reclaim verification in Rust: it recomputes the claim identifier with RFC 8785 canonical JSON and
recovers EIP-191 signatures. Fixtures are signed with the SDK's own `createSignDataForClaim`, and
tests assert byte-for-byte agreement.

**Fail closed.** One forged, foreign, replayed or stale proof aborts the program, so no proof can
exist over a bad input. Valid but irrelevant credentials just don't count.

**Policy as input, hash as output.** One program and verification key serves every Phi industry;
the verifier pins the `policyHash` it accepts.

**EVM wallet binding; chain-agnostic proof.** The brief names no chain. Proofs bind to a 20-byte EVM
address because that is Reclaim's `contextAddress` convention, and SP1's primary on-chain verifier is
Solidity (`contracts/ExpertBadge.sol`). Nothing chain-specific runs inside the proof: the attestor's
secp256k1 signature is checked in the zkVM either way. **Solana path:** bind `contextAddress` to a
base58 ed25519 public key (`parse_address` and `ProgramInput.wallet` → 32 bytes, `address wallet` →
`bytes32` in `PublicValuesStruct`), and verify the Groth16 proof on-chain with Succinct's `sp1-solana`
verifier from an Anchor program instead of `ExpertBadge.sol`.

## What is proven, precisely

For public `(wallet, policyHash, industry, minCount, qualifiedCount, passed, certSetCommitment, evaluatedAt)`,
the prover knows Reclaim proofs such that every proof:
is signed by an attestor in the policy; attests an allowed endpoint of an allowed platform;
is bound to `wallet` and to this app; was attested within `max_age_secs` of `evaluatedAt`;
and exactly `qualifiedCount` distinct ones are ≥ `min_completion_pct` complete and match the
industry, with `certSetCommitment` committing to them under a private random blinding (so the
commitment reveals nothing about which courses, and two wallets with the same course don't match).

## Security tests

| Attack | Fixture / test | Result |
|---|---|---|
| Use someone else's proof | `negative/other-wallet` | `WalletMismatch` |
| Self-signed / rogue attestor | `negative/rogue-attestor` | `UntrustedAttestor` |
| Look-alike domain | `negative/look-alike-host` | `UnknownPlatform` |
| Public page instead of account API | `negative/wrong-endpoint` | `UnknownPlatform` |
| Proof made for another app | `negative/foreign-app-tag` | `WrongContextTag` |
| Old attestation | `negative/stale` | `Stale` |
| Claimant-written extraction spec (fields from other courses) | `negative/unpinned-provider` | `UnpinnedProvider` |
| Self-declared title in `paramValues` | `negative/param-values-title` | `MissingField("title")` |
| App tag that only shares a prefix (`…:v10`) | `negative/tag-prefix` | `WrongContextTag` |
| Edit course title / wallet after signing | tamper tests | signature no longer recovers to attestor |
| Real claim from an attestor the policy doesn't trust | `live/udemy.json` minus our attestor | `UntrustedAttestor` |
| Submit same certificate twice | duplicate fixture | counted once |
| Sneak one bad proof into a good set | fail-closed test | whole evaluation rejected |

## Limitations

The main trust assumption is the attestor set in `policy.json`: our own attestor plus Reclaim's 3 live
attestors (as of 2026-10-28). **In this demo we run the attestor**, so we could sign anything; that is the
same trust model as Reclaim's (a proxy attestor), with a different operator. In production Phi runs its
own attestor(s), ideally several with k-of-n signatures or TEE attestation, and pins them. Industry
matching uses keywords. Details and mitigations in
[docs/DESIGN.md](docs/DESIGN.md#4-known-limitations); provider research in [docs/PROVIDERS.md](docs/PROVIDERS.md).

- **Courses, not certificates.** Udemy's certificate API (`/api-2.0/users/me/certificates/`) exists, but it
  returned an empty list for the test account: both completed courses are free, and free courses report
  `has_certificate: false`. So we prove 100% completion; attesting a real certificate needs a paid course
  and a new request spec for that endpoint. Dedup falls back to the normalized title.
- **One course per proof.** Our request reveals only the most recently accessed course (`page_size=1`).
  N courses need N proofs; pinning one provider hash per `page=N` URL would remove the need to reopen
  courses. `policy.json` (crypto) uses `min_count: 2`, `policy.ai-ml.json` uses `min_count: 1`.
- **Session capture is manual.** The demo takes the session cookie from a "Copy as cURL" file. A product
  would capture it in a browser extension or in-app webview, as Reclaim's app does. The cookie is only
  used inside the TLS session; the attestor sees it redacted.
- **Coursera proves one AI course, not a crypto one.** The test account's one Coursera course is "Building
  Your Own Database Agent", so it earns an AI & ML badge rather than adding to the crypto badge; `policy.json`
  accepts Coursera, it just has no crypto course to count. Completion means `grade.score` = 1 from
  `/api/memberships.v1`, not a certificate. `--start 1..4` are pinned but untested: the account has one
  enrollment. `agent` as an AI keyword is broad, since it would match any title with that word.
- **attestor-core is AGPL-3.0.** Fine for this demo; a product running a modified attestor as a
  service must publish those changes.
- **One deployment per policy.** The contract pins one policy hash, so each industry has its own deployment,
  and changing a policy means a new one. The Sepolia wallet is a throwaway.
