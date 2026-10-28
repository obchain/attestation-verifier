# Design

## 1. Problem

Course platforms already publish certificate links, so "this certificate exists" proves nothing:
anyone can share someone else's link. EulerPhi's Phi platform wants *verified experts* without
collecting personal data. Two things are needed:

1. **zkTLS**: prove a credential came from the platform's own servers (Udemy today; Coursera
   designed, see §4), *from the prover's own logged-in account*, revealing only selected fields.
2. **SP1**: turn several such credentials into one private, checkable statement that matters to
   Phi: *"this identity is a verified expert in industry X."*

## 2. High-level design

```
 ┌──────────── user device ────────────┐          ┌──────────────── prover host ─────────────────┐
 │ Reclaim app / extension             │          │  zkcert CLI (script/)                        │
 │  logs into udemy.com / coursera.org │  proofs  │   load proofs + policy + wallet + time       │
 │  attestor observes TLS session      ├─────────▶│        │                                     │
 │  reveals: title, completion, cert id│  (JSON)  │        ▼                                     │
 │  context: wallet + app tag + nonce  │          │  SP1 zkVM ── program/ ── crates/core         │
 └─────────────────────────────────────┘          │   verify every proof, evaluate policy        │
         ▲ prover/request.ts                      │   commit ABI PublicValuesStruct              │
                                                  └───────────────┬──────────────────────────────┘
                                                                  │ proof + public values
                                                                  ▼
                                       Phi backend / anyone: `zkcert verify`, or an on-chain
                                       SP1 verifier (Groth16/PLONK) → "expert in crypto" badge
```

### Trust model

| Component | Trusted for | If compromised |
|---|---|---|
| Reclaim attestor(s) in `policy.attestors` | Faithfully reporting what udemy.com/coursera.org returned | Can forge credentials. Mitigation: multiple attestors, attestor TEE attestation (SDK supports it), or swap in TLSNotary with a self-run notary. |
| Udemy / Coursera | Truth of the data | Out of scope |
| SP1 | Soundness of the zkVM proof | Out of scope |
| Prover host | Nothing | Cannot forge: all checks run inside the zkVM. It can only choose *which* valid credentials to include and the evaluation time (which is public). |
| Verifier | Checking program vkey + policy hash | Must pin both, or a different program/policy could be passed off |

## 3. Low-level design

### 3.1 Reclaim proof (input)

```jsonc
{
  "claimData": {
    "provider": "http",
    "parameters": "{\"method\":\"GET\",\"url\":\"https://www.udemy.com/api-2.0/...\",\"responseMatches\":[...]}",
    "context": "{\"contextAddress\":\"0xWALLET\",\"contextMessage\":\"eulerphi-zkcert:v1:<nonce>\",\"extractedParameters\":{\"title\":\"...\",...},\"providerHash\":\"0x..\"}",
    "owner": "0x...", "timestampS": 1793167200, "epoch": 1, "identifier": "0x..."
  },
  "signatures": ["0x<65 bytes r||s||v>"],
  "witnesses": [{ "id": "0xattestor", "url": "wss://..." }]
}
```

Signature scheme (matches `@reclaimprotocol/js-sdk` 5.8.3, verified by tests):

```
identifier = keccak256(provider "\n" parameters "\n" JCS(context))       // RFC 8785
signed     = "0x"+hex(identifier) "\n" lower(owner) "\n" timestampS "\n" epoch
signer     = ecrecover(EIP191(signed), sig)
```

The `identifier` field in the proof is ignored and always recomputed, so any edit to the URL,
extracted fields, wallet or tag changes what was "signed" and the signature stops recovering to
the attestor.

### 3.2 Per-proof checks (`crates/core::verify_proof`)

| # | Check | Defeats |
|---|---|---|
| 1 | ≥1 signature recovers to an address in `policy.attestors` | Forged/edited proofs, self-signed proofs |
| 2 | `parameters.url` is `https://`, host ∈ platform `hosts`, path starts with a pinned prefix | Proofs from look-alike domains (`udemy.com.evil.io`), or from public pages anyone can load |
| 2b | Provider hash of `{url, method, body, responseMatches, responseRedactions}` (the SDK's `hashProofClaimParams`) ∈ platform `provider_hashes` | A claimant writing their own extraction spec, e.g. title from one course and completion from another, or a path-traversal URL |
| 3 | `context.contextAddress == wallet` | Replaying someone else's proof |
| 4 | `context.contextMessage` is `context_tag` or starts with `context_tag + ":"` | Reusing a proof the user made for a different app or app version |
| 5 | `now - timestampS ≤ max_age_secs` (+5 min skew) | Stale or revoked credentials lingering forever |
| 6 | Title present; completion parsed (or implied for certificate-only endpoints), read only from attestor-extracted `context.extractedParameters`, never from claimant `paramValues` | Malformed claims, self-declared fields |

### 3.3 Set evaluation (`crates/core::evaluate`)

- `dedup_key = sha256(platform ‖ 0x00 ‖ ("id:"+cert_id | "title:"+normalized_title))`
- Each verified credential gets a verdict: `Counted | Duplicate | Incomplete | OtherIndustry`.
- Industry match: word-boundary keyword match on the title (`"defi"` ≠ `"definitely"`; `zk*` prefix).
- `certSetCommitment = sha256(sorted dedup keys of counted credentials)` (order-independent).
  The holder can later reveal one credential and prove it's in the set without re-proving.
- Any rejected proof aborts with its index: **fail closed**.

### 3.4 Public values (output, ABI-encoded)

```solidity
struct PublicValuesStruct {
  address wallet; bytes32 policyHash; string industry;
  uint32 minCount; uint32 qualifiedCount; bool passed;
  bytes32 certSetCommitment; uint64 evaluatedAt;
}
```

`policyHash = sha256(JSON(policy))` covers attestors, endpoints, field mapping, keywords and
thresholds, so a verifier knows exactly which rules were applied.

### 3.5 Why the policy is an input rather than hard-coded

Phi has many industries and attestor sets rotate. Making the policy an input keeps one program
(one vkey) for all of them; the verifier pins the `policyHash` it accepts per industry.

## 4. Known limitations

- **Attestor trust** (see table). This is the main assumption of any proxy/attestor zkTLS design.
- **Field mapping is per provider.** `policy.json` pins the "Udemy Course Name" provider's endpoint
  (`/api-2.0/users/me/subscribed-courses/`) and fields (`title`, `completion_ratio`), taken from the
  provider's definition (docs/PROVIDERS.md). Any other provider needs its own policy block.
- **Courses vs certificates.** That provider exposes course title and completion ratio, with no
  certificate id or date, so the claim is "completed courses". A custom provider on Udemy's
  certificate endpoint would be stronger.
- **One course per proof.** The provider reveals only `results[0]`, the most recently accessed
  course. N courses need N proofs; a custom provider with more indices would fix this.
- **Coursera is designed, not shipped.** No public Reclaim provider exists. A custom provider on the
  logged-in accomplishments API is needed (not the public verify page, which anyone holding the
  code could attest). The policy block exists; no Coursera proof has been made.
- **Keyword industry mapping is naive.** A curated course-ID → industry map, or issuer-side
  tags, would be better.
- **Evaluation time is chosen by the prover** (but public). A verifier should reject
  `evaluatedAt` far from its own clock, or the on-chain contract should compare to `block.timestamp`.
- **JCS edge cases**: key sorting by UTF-8 bytes equals RFC 8785's UTF-16 ordering only for
  ASCII keys (all Reclaim keys are ASCII); non-integer numbers in context aren't expected.

## 5. Future work

- TLSNotary backend with a self-run notary (removes dependency on Reclaim's attestors).
- Require k-of-n attestors; verify attestor TEE attestation reports.
- On-chain `ExpertBadge` contract: `verify(publicValues, proof)` via SP1's Groth16 verifier,
  pin vkey + policyHash per industry, check `evaluatedAt` vs `block.timestamp`, mint a
  soulbound badge to `wallet`.
- More Phi proofs from the same pipeline: employment (LinkedIn/Deel), eligibility (18+,
  region) with a KYC source.
