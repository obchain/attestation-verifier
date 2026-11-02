# Plan and status

Original deadline Thu 2026-10-29; extended to finish the live pipeline (2026-10-31/11-01).

## Done (Wed 28 Oct)

- [x] Confirmed Reclaim's exact signing format from `@reclaimprotocol/js-sdk` 5.8.3 source
- [x] `crates/core`: proof verification, policy, industry matching, dedup, commitment, ABI output
- [x] Fixtures signed through the SDK's own signing functions + 26 passing tests
      (SDK compatibility, happy path, 6 negative fixtures, tamper tests, fail-closed)
- [x] SP1 guest program + `zkcert` CLI (`check | execute | prove | verify | vkey`)
- [x] `prover/request.ts` (real wallet-bound Reclaim proofs), `sdk-verify.ts` cross-check
- [x] README, DESIGN, this plan, CLAUDE.md

## Needs a human (candidate)

- [x] Reclaim dev account: APP_ID / APP_SECRET + Udemy Course Name provider → `prover/.env`
- [x] Udemy account with ≥1 completed course whose title matches the industry
- [x] Install SP1: `curl -L https://sp1up.succinct.xyz | bash && sp1up --version 6.10.0` (+ `brew install protobuf`)

## Next (Wed evening → Thu)

1. [x] `cd script && cargo run --release -- execute --input ../fixtures/input.test.json`
   (only fix needed: Rust pin 1.90 → 1.91.1 for SP1 6.10 host crates)
2. [x] `cargo run --release -- prove --input ../fixtures/input.test.json --system compressed`, then `verify`
   — SP1 6.10.0, 1,110,897 cycles, compressed prove 146 s wall on M-series (19.5 GB peak RSS),
   proof.bin 1.27 MB, vkey `0x00f7542b4a7d932944f31c68acde3fa578798ee1410bb62bf3754f5ee975a58b`.
   Rogue-attestor fixture panics the guest in the zkVM (fail closed).
   After the security-review fixes (provider hash, blinded commitment): 1,705,314 cycles.
3. [x] `policy.json`: 3 live attestors, Udemy endpoint + fields pinned, `min_count: 1` (docs/PROVIDERS.md)
4. [x] Real Udemy proof. Reclaim's hosted provider turned out stale and its builder was down
   (docs/PROVIDERS.md §0), so: self-hosted attestor-core + `prover/src/attest.ts` with our own
   request spec. Live claim 2026-10-31, committed as `fixtures/live/udemy.json`.
5. [x] `sdk-verify`: SDK recomputes the identifier; `completion_ratio` is 0–100 (`100`).
6. [x] `zkcert check` → COUNTED, `passed true`; `prove --system compressed` 76 s; `verify` VALID
   (vkey `0x00ab7879f5ca22f155c50600e4209ac6b8e93c3e1eac1b42b7cbde0ac99439b8`).
7. [ ] Coursera live: same approach (own request spec on the logged-in accomplishments API).
   Needs a test account with a completed course.
8. [x] README/DEMO/DESIGN rewritten for the live flow. [ ] Record the 2–3 min demo (docs/DEMO.md)
10. [x] Second completed crypto course → live `min_count: 2`, `qualifiedCount 2` on Sepolia (2026-11-02).
    Real certificate: needs a paid course (free courses have none), not done.
11. [ ] Cleanup: log out of Udemy (cookies were shared in chat), delete `~/udemy.curl`,
    rotate the Reclaim app secret, remove merged track branches/worktrees.
9. [x] `ExpertBadge.sol` + 10 Foundry tests (mock verifier). [x] Groth16 + Sepolia: `0x441f21eff2c97d19c1aac3550e5c561f8fa58692`,
   claim tx `0xf56dc1f4f64f70711360a8a7097c1e9c51218b17c71cca639a8118a91987a9f0`, `hasBadge = true` (2026-11-02). Evidence in `fixtures/live/onchain.json`.
