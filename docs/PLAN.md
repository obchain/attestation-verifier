# Plan and status

Deadline: **Thu 2026-10-29, end of day IST**.

## Done (Wed 28 Oct)

- [x] Confirmed Reclaim's exact signing format from `@reclaimprotocol/js-sdk` 5.8.3 source
- [x] `crates/core`: proof verification, policy, industry matching, dedup, commitment, ABI output
- [x] Fixtures signed through the SDK's own signing functions + 19 passing tests
      (SDK compatibility, happy path, 6 negative fixtures, tamper tests, fail-closed)
- [x] SP1 guest program + `zkcert` CLI (`check | execute | prove | verify | vkey`)
- [x] `prover/request.ts` (real wallet-bound Reclaim proofs), `sdk-verify.ts` cross-check
- [x] README, DESIGN, this plan, CLAUDE.md

## Needs a human (candidate)

- [ ] Reclaim dev account: APP_ID / APP_SECRET, pick Udemy + Coursera provider IDs → `prover/.env`
- [ ] Udemy account with ≥1 completed paid course (ideally Coursera certificate too)
- [x] Install SP1: `curl -L https://sp1up.succinct.xyz | bash && sp1up --version 6.10.0` (+ `brew install protobuf`)

## Next (Wed evening → Thu)

1. [x] `cd script && cargo run --release -- execute --input ../fixtures/input.test.json`
   (only fix needed: Rust pin 1.90 → 1.91.1 for SP1 6.10 host crates)
2. [x] `cargo run --release -- prove --input ../fixtures/input.test.json --system compressed`, then `verify`
   — SP1 6.10.0, 1,110,897 cycles, compressed prove 146 s wall on M-series (19.5 GB peak RSS),
   proof.bin 1.27 MB, vkey `0x00f7542b4a7d932944f31c68acde3fa578798ee1410bb62bf3754f5ee975a58b`.
   Rogue-attestor fixture panics the guest in the zkVM (fail closed).
3. Real Udemy proof: `cd prover && npm run request -- --platform udemy --wallet 0x...`
4. `npm run sdk-verify -- ../proofs/<file>.json --online` → copy attestor address into
   `policy.json`, confirm field names + endpoint path, tighten `path_prefixes`
5. `zkcert check --proofs-dir proofs --policy policy.json --wallet 0x...` then `prove`
6. Coursera: timebox 2h (public provider or custom provider). Document if partial.
7. Record 2–3 min demo; final README pass.
8. Stretch (only if done by ~3 pm Thu): Groth16 + `ExpertBadge.sol` on a testnet.
