# Demo script (2–3 min)

Two terminals side by side, both at the repo root, font large. SP1 on PATH
(`export PATH="$PATH:$HOME/.sp1/bin"`). Run `cargo build --release` in `script/` beforehand so
nothing compiles on camera. Before recording: start the attestor in a third terminal
(README step 3), keep Udemy logged in, and save a fresh `~/udemy.curl`. The policy needs 2 courses:
make one claim off camera first (open one completed course on Udemy, run `npm run attest`), then open the other. Clear old claims with
`rm -f proofs/*.json` before that first claim, so only today's claims are used.

Set once:

```bash
export WALLET=0xYourWallet
export BLINDING=0x$(openssl rand -hex 32)   # private; same value keeps check and prove consistent
```

## 0. Problem (15 s, no terminal)

> Phi wants verified experts per industry without collecting personal data. A Udemy certificate
> link proves nothing — anyone can share someone else's. This repo proves, in zero knowledge:
> *this wallet completed courses in the Crypto industry, attested from udemy.com's own servers*,
> and reveals nothing else.

## 1. zkTLS: get a wallet-bound claim from Udemy (40 s)

Show the attestor terminal briefly (`WS server listening`, `signerAddress 0x3dafd19a…`).

```bash
cd prover && npm run attest -- --wallet $WALLET
```

> This is Reclaim's open-source attestor, run by us. My machine opens a TLS session to udemy.com
> *through* it, with my own session cookie. The attestor never sees the cookie: it gets ZK proofs that
> the bytes it's shown really decrypt to Udemy's response, and signs only the course title and
> completion ratio. My wallet and our app tag `eulerphi-zkcert:v1` are inside the signed claim, so
> nobody can replay it for another wallet or app.

Point at `revealed fields: { completion_ratio: '100', title: '…Blockchain…' }`.

> Why our own attestor? Reclaim's public Udemy provider is stale: Udemy changed its API request and
> the hosted flow never fires. Running the attestor ourselves took an afternoon and changed nothing in
> the verifier.

## 2. Cross-check with Reclaim's own SDK (15 s)

```bash
npm run sdk-verify -- ../proofs/udemy-*.json
```

> Reclaim's SDK recomputes the same claim identifier and signer, so this is a standard Reclaim claim.
> But we don't rely on it: the trust boundary is the zkVM.

## 3. Native dry run (20 s)

```bash
cd ../script && cargo run --release -q -- check --proofs-dir ../proofs --policy ../policy.json --wallet $WALLET --blinding $BLINDING
```

> Same Rust verifier that runs inside SP1: checks the attestor signature, host and endpoint,
> wallet binding, freshness, completion and the industry keywords. The course title is visible
> here only because this is my machine. Below the line is everything a verifier will learn.

## 4. Prove in SP1 (30 s on camera, ~2.5 min real)

```bash
cargo run --release -q -- prove --proofs-dir ../proofs --policy ../policy.json --wallet $WALLET --blinding $BLINDING --system compressed
```

Cut or speed up the wait (about 80 s). Mention keccak/secp256k1 precompiles.

## 5. Anyone can verify (15 s)

```bash
cargo run --release -q -- verify --proof proof.bin
```

> The verifier sees: wallet, industry `crypto`, qualified count, `passed: true`, the policy hash
> (which attestors, endpoints and thresholds were applied), a commitment to the course set, and the
> evaluation time. No course names, no account, no email. The commitment is blinded, so it can't be
> matched against Udemy's public course list.

## 5b. On-chain (20 s)

Open the [claim tx](https://sepolia.etherscan.io/tx/0xf56dc1f4f64f70711360a8a7097c1e9c51218b17c71cca639a8118a91987a9f0) on Etherscan, then:

```bash
cast call 0x441f21eff2c97d19c1aac3550e5c561f8fa58692 "hasBadge(address,string)(bool)" 0xDAcEa3F3ef21eC7A525591810FcD4615D5154431 crypto --rpc-url https://ethereum-sepolia-rpc.publicnode.com
```

> Same pipeline with a Groth16 wrapper: Succinct's live verifier on Sepolia accepted the proof and the
> contract recorded the badge. The chain sees the verdict and a blinded commitment, no course names.
> (Pre-made: a Groth16 run is too slow to film.)

## 6. It fails closed (20 s)

```bash
mkdir -p /tmp/rogue && cp ../fixtures/negative/rogue-attestor.json /tmp/rogue/
cargo run --release -q -- check --proofs-dir /tmp/rogue --policy ../fixtures/policy.test.json \
  --wallet 0x5Aae27DacAf8b6b7A6eEA2406bf356F105cae7C5 --at 1793170800
```

Expected: `proof #0 rejected: no signature from a trusted attestor`.

> A self-signed proof is rejected, and inside the zkVM the program aborts, so no proof can exist.
> `cargo test` covers six more attacks: other wallet, look-alike host, wrong endpoint, foreign app,
> stale proof, tampered fields.

## 7. Close (15 s)

> Scope: Udemy is live end to end. Coursera uses the same pipeline with its own request spec.
> `ExpertBadge` is live on Sepolia.
> Main trust assumption: the attestors pinned in the policy. Here that's us; in production, Phi's own
> attestors, ideally several.

## Fallback: Udemy session expired or attestor down

Use the committed real claims: run steps 3–5 with `--proofs-dir ../fixtures/live/claims
--wallet 0xDAcEa3F3ef21eC7A525591810FcD4615D5154431 --at 1793609751` (made 2026-11-02; `--at` keeps them
inside the freshness window). Last resort:
`--input ../fixtures/input.test.json` (test attestor, signed through Reclaim's SDK code).
