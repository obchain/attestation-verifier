# CLAUDE.md — context for anyone (human or Claude Code) picking up this repo

## The assignment (take-home for EulerPhi)

> Build a zkTLS system to verify certifications from platforms such as Coursera and
> Udemy. Additionally, develop a proving system using SP1 to prove a claim of your
> choice that is relevant to EulerPhi.

Deadline: **Thursday 2026-10-29, end of day IST.** Scope is deliberately cut to fit (see docs/PLAN.md).

## What EulerPhi is (from eulerphi.com, read 2026-10-28)

- Main product **Phi**: a SocialFi "trust layer" connecting verified experts, companies and
  information, using ZK proofs so users can trust what they see without handing over data.
- Organized by **industries** (Crypto, AI & ML, Biotech, Cybersecurity, Medicine). One account,
  one reputation, different context per industry. Experts have verified track records.
- "Be Unique" proofs: human, identity, **education (degrees + professional certifications)**,
  employment, digital assets, privacy/eligibility (18+, region, sanctions), media authenticity.
- Identities look like `did:phi:…`. Longer-term goal: tokenized private-company shares.

## Decisions already made (don't relitigate without reason)

1. **zkTLS = Reclaim Protocol**, not TLSNotary. Reason: deadline. Reclaim already has Udemy
   providers, and its proofs are secp256k1 EIP-191 attestor signatures, which are cheap to verify
   inside SP1 (keccak + secp256k1 precompiles). TLSNotary presentations would require porting
   `tlsn-core` verification into the zkVM, an unknown we couldn't afford. Trade-off (stated in README):
   we trust Reclaim's attestor set instead of a notary.
2. **We do not call Reclaim's `verifyProof` as the trust boundary.** `crates/core` reimplements
   verification in Rust (identifier recomputation, RFC 8785 context canonicalization, EIP-191
   recovery). Fixtures are signed with the SDK's own `createSignDataForClaim`, and tests prove our
   Rust matches it byte for byte.
3. **SP1 claim = "verified expert in a Phi industry"**: wallet W holds ≥ N distinct, fully
   completed Udemy/Coursera credentials whose titles match industry X, attested by trusted
   attestors, fresh within T. Course names and account data stay private.
4. **Wallet binding** via Reclaim `context.contextAddress`; **domain separation** via
   `contextMessage` starting with `eulerphi-zkcert:v1`. Both are inside the signed identifier.
5. **Fail closed**: one invalid proof aborts the SP1 program (no proof can be made). Proofs that
   are valid but don't qualify (other industry, incomplete, duplicate) just aren't counted.
6. Public values are **ABI-encoded** (`PublicValuesStruct`) so a Solidity verifier can decode them.
7. On-chain verifier contract and frontend are **stretch goals**, not required by the brief.

## Repo map

- `crates/core` — ALL verification logic, shared by guest and host. Start here.
- `program` — SP1 guest (tiny: read input → `evaluate` → commit ABI public values).
- `script` — host CLI `zkcert` (`check | execute | prove | verify | vkey`). Needs SP1 toolchain.
- `prover` — TypeScript: `attest.ts` (live claim on our self-hosted attestor), `request.ts` (Reclaim hosted flow, currently broken upstream), `fixtures.ts`
  (test fixtures signed by a throwaway attestor via the SDK), `sdk-verify.ts` (SDK cross-check).
- `fixtures/` — generated test data. `policy.test.json` trusts ONLY the test attestor.
- `policy.json` — real policy. Trusts our self-hosted attestor `0x3dafd19a…5485` plus the 3 live Reclaim
  attestors (2026-10-28); pins two Udemy provider hashes (Reclaim's stale one and our `attest.ts` spec);
  `min_count: 1` for the live demo.
- `fixtures/live/udemy.json` — a real Udemy claim (2026-10-31) from our attestor; tested against `policy.json`.
- `../attestor-core` (outside the repo) — Reclaim's attestor server, commit `dd43dc8`; key in its
  `.env.development` (never commit). Start: `node --experimental-strip-types src/scripts/start-server.ts`.
- `contracts/` — `ExpertBadge.sol` (stretch) + Foundry tests against a mock SP1 verifier. Not deployed.
- `docs/PROVIDERS.md` — Reclaim provider IDs, endpoints, fields, attestor list (research, with sources).
- `docs/DESIGN.md` — HLD, LLD, threat model. `docs/PLAN.md` — status and next steps.

## Conventions

- Keep `crates/core` free of anything zkVM-specific so it is unit-testable natively.
- Crypto crate versions are pinned to the ones SP1 6.10 patches (k256 0.13.4, sha2 0.10.8,
  tiny-keccak 2.0.2). Don't bump them casually.
- Do NOT enable serde_json `preserve_order`: canonical JSON depends on sorted BTreeMap keys.
- Every new rejection path gets a negative fixture + test.
- `cargo test` (root) runs core tests without the SP1 toolchain. Rust is pinned to 1.91.1
  (`rust-toolchain.toml`, needed by SP1 6.10 host crates); the guest builds on `succinct`.

## Decisions made 2026-10-28 (after research, see docs/PROVIDERS.md)

8. **Udemy only is shipped; Coursera is a design.** No public Coursera provider exists on Reclaim;
   a custom provider would need Reclaim review, which doesn't fit the deadline.
9. **Claim wording is "completed courses", not "certificates".** The "Udemy Course Name" provider
   (`3ef2059d-ec1e-4107-8187-e35157e527ad`) exposes `title` + `completion_ratio` only, and only for the
   most recently accessed course (one course per proof). Dedup falls back to normalized title.
10. **`policy.json` `min_count: 1`** for the live demo (one completed course available). Test
    policy keeps 2.

## Decisions made 2026-10-31

11. **Self-hosted attestor for live claims.** Reclaim's hosted Udemy provider is stale (Udemy's page no
    longer makes the templated request) and its provider builder was down. We run Reclaim's open-source
    attestor-core and send our own request spec (`attest.ts`). Same claim format, so `crates/core` is
    unchanged. Trust model stated plainly: in the demo we are the attestor. Evidence in docs/PROVIDERS.md §0.
12. **Changing `UDEMY_PARAMS` in `attest.ts` changes the provider hash**; re-pin it in `policy.json`
    (run `npm run sdk-verify` on a new claim to print it). Secrets (cookie, UA) go in `secretParams` only,
    so the hash is the same for every user.

## Open items

- Coursera live claim (needs a request spec + test account).
- Rust ↔ Solidity `PublicValuesStruct` encoding is not cross-checked in a test (deliberately skipped).
