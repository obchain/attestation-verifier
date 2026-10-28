# eulerphi-zkcert

**Verified-expert proofs for Phi, from zkTLS-attested Udemy course completions, proven in SP1.**
Coursera uses the same pipeline but needs a custom Reclaim provider (see [Limitations](#limitations)).

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
Udemy / Coursera ──TLS──▶ Reclaim attestor ──signed claim──▶ proofs/*.json
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
| `prover` | TypeScript zkTLS side: request real proofs, generate fixtures, SDK cross-check |
| `fixtures` | Test proofs signed via the Reclaim SDK with a throwaway test attestor |

## Quick start

**1. Tests (no SP1 toolchain needed)**

```bash
cargo test            # 19 tests: SDK compatibility, happy path, forgery/replay/staleness rejections
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

**3. Real credentials**

```bash
cd prover && npm install && cp .env.example .env   # fill in Reclaim app + provider IDs
npm run request -- --platform udemy --wallet 0xYourWallet     # scan QR, log into Udemy
npm run sdk-verify -- ../proofs/udemy-*.json --online          # signer must be one of policy.json "attestors"
cd ../script
cargo run --release -- prove --proofs-dir ../proofs --policy ../policy.json --wallet 0xYourWallet
```

## Design choices

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

## What is proven, precisely

For public `(wallet, policyHash, industry, minCount, qualifiedCount, passed, certSetCommitment, evaluatedAt)`,
the prover knows Reclaim proofs such that every proof:
is signed by an attestor in the policy; attests an allowed endpoint of an allowed platform;
is bound to `wallet` and to this app; was attested within `max_age_secs` of `evaluatedAt`;
and exactly `qualifiedCount` distinct ones are ≥ `min_completion_pct` complete and match the
industry, with `certSetCommitment` committing to them.

## Security tests

| Attack | Fixture / test | Result |
|---|---|---|
| Use someone else's proof | `negative/other-wallet` | `WalletMismatch` |
| Self-signed / rogue attestor | `negative/rogue-attestor` | `UntrustedAttestor` |
| Look-alike domain | `negative/look-alike-host` | `UnknownPlatform` |
| Public page instead of account API | `negative/wrong-endpoint` | `UnknownPlatform` |
| Proof made for another app | `negative/foreign-app-tag` | `WrongContextTag` |
| Old attestation | `negative/stale` | `Stale` |
| Edit course title / wallet after signing | tamper tests | signature no longer recovers to attestor |
| Submit same certificate twice | duplicate fixture | counted once |
| Sneak one bad proof into a good set | fail-closed test | whole evaluation rejected |

## Limitations

The main trust assumption is the Reclaim attestor set (`policy.json` pins the 3 live attestors as of
2026-10-28). Industry matching uses keywords. Details and mitigations in
[docs/DESIGN.md](docs/DESIGN.md#4-known-limitations); provider research in [docs/PROVIDERS.md](docs/PROVIDERS.md).

- **Courses, not certificates.** Reclaim's public Udemy provider ("Udemy Course Name") exposes title and
  `completion_ratio`, not a certificate id or date, so "completed" means 100% progress. Dedup falls back
  to the normalized title.
- **One course per proof.** That provider reveals only the most recently accessed course, so N courses
  need N proofs, each made after opening a different completed course. The real `policy.json` uses
  `min_count: 1` for the live demo (one completed course was available); the test policy and fixtures
  keep `min_count: 2` to exercise dedup and thresholds. Raising it is a one-line policy change.
- **Coursera is designed, not shipped.** No public Reclaim provider exists; a custom provider on the logged-in
  accomplishments API is needed. The policy block is in place, but no Coursera proof has been made.
- **Solidity ↔ Rust encoding not cross-checked.** `contracts/` tests use Solidity-encoded public values;
  decoding a Rust-produced `PublicValuesStruct` in a test is still to do. Nothing is deployed.
