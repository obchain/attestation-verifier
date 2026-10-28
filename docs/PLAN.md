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
- [ ] Install SP1: `curl -L https://sp1up.succinct.xyz | bash && sp1up`

## Next (Wed evening → Thu)

1. `cd script && cargo run --release -- execute --input ../fixtures/input.test.json`
   (first SP1 build; fix anything the toolchain complains about)
2. `cargo run --release -- prove --input ../fixtures/input.test.json --system compressed`, then `verify`
3. Real Udemy proof: `cd prover && npm run request -- --platform udemy --wallet 0x...`
4. `npm run sdk-verify -- ../proofs/<file>.json --online` → copy attestor address into
   `policy.json`, confirm field names + endpoint path, tighten `path_prefixes`
5. `zkcert check --proofs-dir proofs --policy policy.json --wallet 0x...` then `prove`
6. Coursera: timebox 2h (public provider or custom provider). Document if partial.
7. Record 2–3 min demo; final README pass.
8. Stretch (only if done by ~3 pm Thu): Groth16 + `ExpertBadge.sol` on a testnet.
