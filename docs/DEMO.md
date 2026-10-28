# Demo script (2–3 min)

Two terminals side by side, both at the repo root, font large. SP1 on PATH
(`export PATH="$PATH:$HOME/.sp1/bin"`). Run `cargo build --release` in `script/` beforehand so
nothing compiles on camera.

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

## 1. zkTLS: get a wallet-bound proof from Udemy (40 s)

```bash
cd prover && npm run request -- --platform udemy --wallet $WALLET
```

Scan the QR code on your phone, log into Udemy in the Reclaim flow.

> The Reclaim attestor watches the TLS session to udemy.com and signs only the fields we asked for:
> course title and completion ratio. My wallet and our app tag `eulerphi-zkcert:v1` are inside the
> signed claim, so nobody can replay this proof for another wallet or app.

Show the saved file briefly: `ls ../proofs/`.

## 2. Cross-check with Reclaim's own SDK (15 s)

```bash
npm run sdk-verify -- ../proofs/udemy-*.json --online
```

> Reclaim's SDK agrees the signature is valid and the signer is in the live attestor list.
> But we don't rely on it — the trust boundary is the zkVM.

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

Cut or speed up the wait. Mention ~1.7M cycles, keccak/secp256k1 precompiles.

## 5. Anyone can verify (15 s)

```bash
cargo run --release -q -- verify --proof proof.bin
```

> The verifier sees: wallet, industry `crypto`, qualified count, `passed: true`, the policy hash
> (which attestors, endpoints and thresholds were applied), a commitment to the course set, and the
> evaluation time. No course names, no account, no email. The commitment is blinded, so it can't be
> matched against Udemy's public course list.

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

> Scope: Udemy is live. Coursera uses the same pipeline but needs a custom Reclaim provider.
> `contracts/ExpertBadge.sol` is the on-chain path once we produce a Groth16 proof.
> Main trust assumption: Reclaim's attestor set.

## Fallback: no real proof

Run steps 3–5 with `--input ../fixtures/input.test.json` instead of `--proofs-dir/--policy/--wallet`,
and say plainly that the fixtures are signed by a test attestor through Reclaim's own SDK signing code.
