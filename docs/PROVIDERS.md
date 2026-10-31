# Reclaim providers, attestors and `policy.json` (research, 2026-10-28)

Research only; nothing in `policy.json` was changed. Confidence: **confirmed** (seen in a live API response / SDK source today), **likely**, **guess**.
Provider JSON was read from Reclaim's public API `GET https://api.reclaimprotocol.org/api/providers/<id>` (no auth needed).

## 0. Update 2026-10-31: the hosted provider is stale

Sections 1–4 are the 2026-10-28 research. Trying it live showed that Reclaim's hosted flow cannot work today.

**Evidence** (all confirmed, 2026-10-31):

| What | Observed |
|---|---|
| Provider `3ef2059d…` request (API JSON, `urlType: TEMPLATE`, `expectedPageUrl: /home/my-courses/`) | `subscribed-courses/?ordering=-last_accessed&fields[course]=archive_time,buyable_object_type,completion_ratio,enrollment_time,favorite_time,features,image_240x135,image_480x270,is_practice_test_course,is_private,is_published,last_accessed_time,num_collections,published_title,title,tracking_id,url,visible_instructors&fields[user]=@min,job_title&page=1&page_size=12&is_archived=false` |
| What Udemy's My Learning page requests now (browser network log, desktop **and** mobile viewport) | `subscribed-courses/?ordering=-last_accessed&fields[course]=archive_time,favorite_time,features,is_private,num_collections,tracking_id,url&page=1&page_size=12&is_archived=false`: no `title`, no `completion_ratio` |
| Effect | The provider waits for a request that never happens; the Reclaim session sits on "Udemy running" forever. Not fixable from our side (no wrong page, account or mode) |
| Other Udemy providers (`d787d917…` Course Count, `79c5741c…` Last Purchase) | Same stale URL, or the wrong data |
| Reclaim provider builder (to make a corrected provider) | `ClientError: /channel.SessionService/GenerateSession UNKNOWN: Response closed without headers`, on 2026-10-30 and again 2026-10-31 |
| Plain `curl` to the API with only `access_token` | `403` Cloudflare "Just a moment…" |

**What we did instead.** Ran Reclaim's open-source attestor-core ourselves and sent our own request spec
(`prover/src/attest.ts`):

```
GET https://www.udemy.com/api-2.0/users/me/subscribed-courses/?ordering=-last_accessed&page_size=1&fields[course]=title,completion_ratio
responseMatches:    "title":"(?<title>…)"   "completion_ratio":(?<completion_ratio>\d+)
responseRedactions: $.results[0].title, $.results[0].completion_ratio
```

Udemy answered `200` through the attestor: Cloudflare passed with the full browser cookie set and User-Agent.
Response returned: `{"title":"Learn the secrets of Blockchain in less than an hour","completion_ratio":100}`, so
`completion_ratio` is a 0–100 integer (resolves the "likely" in section 1). Provider hash of our spec:
`0x1343d0b39a5c913c300ec51fec24ce816a2edabb08981b73b968c83398ff489c` (pinned in `policy.json`).
Our attestor: `0x3dafd19a94d459d99b5154e8da32c932889d5485`.

Also seen: the course player calls `/api-2.0/users/me/subscribed-courses/<id>/progress/` (has
`first_completion_time`, a better "completed at" date) and the public `/api-2.0/courses/<id>/?fields[course]=title`.
A stronger future spec could attest progress + date and map course ID → title separately.

## 1. Udemy providers — found

| Provider | ID | Active / approved | Conf. |
|---|---|---|---|
| Udemy Course Name | `3ef2059d-ec1e-4107-8187-e35157e527ad` v1.0.0 | yes / yes, `usedInCount` 8 | confirmed |
| Udemy – Course Count – Game of Life | `d787d917-204b-4464-95c4-b4f0ecb213a8` | yes / yes | confirmed |
| Udemy Last Purchase | `79c5741c-a0fc-4037-99ff-007f58faffa0` | yes / yes | confirmed |

Sources: [Course Name page](https://dev.reclaimprotocol.org/provider/details/3ef2059d-ec1e-4107-8187-e35157e527ad), [Last Purchase page](https://dev.reclaimprotocol.org/provider/details/79c5741c-a0fc-4037-99ff-007f58faffa0), [Course Count page](https://dev.reclaimprotocol.org/provider/details/d787d917-204b-4464-95c4-b4f0ecb213a8), and the API JSON above.

**Course Name** (the useful one) — confirmed from API JSON:
- URL (TEMPLATE, GET): `https://www.udemy.com/api-2.0/users/me/subscribed-courses/?ordering=-last_accessed&fields[course]=...completion_ratio...title...&page=1&page_size=12&is_archived=false`
- `responseMatches` / extracted variables: `title`, `image_480x270`, `completion_ratio` (raw number, regex `"completion_ratio":(.*)`).
- Redaction jsonPaths: `$.results[0].title`, `$.results[0].completion_ratio` → **only the first result** (most recently accessed course, per `ordering=-last_accessed`) is revealed.
- These variable names match the existing `title_keys` / `completion_keys` in `policy.json` (`title`, `completion_ratio`). Good.
- Udemy's `completion_ratio` is a 0–100 integer in the Udemy API (**likely**, not seen in a real response) → `completion_is_fraction: false` stays.

**Answer to "certificate endpoint?": no.** None of the three targets a certificate endpoint. Not found: any Udemy provider that returns certificate id/date. All three are course-progress / purchase data. (Search of dev.reclaimprotocol.org via web search; the API has no public list/search endpoint — `/api/providers` returns "Not found" — so absence is **likely**, not proven.)

**Consequences for us**
1. One proof = one course (the latest accessed). A user needing N=2 must produce 2 proofs *after changing which course was last accessed* — awkward. `page_size` in the URL is fixed by the provider template; a custom provider with `$.results[1]` etc. (or one provider per index) is needed for real multi-course use. (confirmed from redaction paths)
2. `completion_ratio` 100 ≠ "certificate earned"; it is the course progress bar. Brief says "certifications"; honest framing = "completed courses". A custom provider on Udemy's certificate listing would be better but the endpoint is unknown to me: **not found**.
3. `cert_id_keys` (`certificate_id`…) will never match for these providers; dedup then falls back to whatever `lib.rs` does without an id — check that it dedups on title (or Track A should).
4. `host`/path in `policy.json` already fit: host `www.udemy.com`, path `/api-2.0/users/me/subscribed-courses/` (path only, query is not part of `path`, per `split_url` in `crates/core/src/lib.rs:112`).

## 2. Coursera — no public provider found

- No Coursera provider turned up in web searches of dev.reclaimprotocol.org or Reclaim docs (only generic universities and the three Udemy ones). **Not found** (likely absent). Sources: [search results](https://dev.reclaimprotocol.org/provider/details/3ef2059d-ec1e-4107-8187-e35157e527ad) list only Udemy; Reclaim [education page](https://reclaimprotocol.org/education-verification) mentions universities, not Coursera.
- Custom provider path: Reclaim dev portal → create HTTP provider; reviewed/activated manually ([Burnt guide](https://docs.burnt.com/xion/developers/verification/internet-verification/how-to-create-a-custom-reclaim-zktls-provider), [Reclaim wtf-are-providers](https://docs.reclaimprotocol.org/wtf-are-providers)). Confirmed that process exists; timeline for approval unknown.
- Official Coursera API is partner-only (`api.coursera.org`, OAuth client credentials) → useless for a user-session proof ([third-party summary](https://www.rapidevelopers.com/md/bubble-integrations/coursera), likely). A provider must replay the **first-party web API** under `www.coursera.org`.
- Endpoint candidates (**all guess**; I could not find documented first-party endpoints — [Unbrowse page](https://smithery.ai/skills/unbrowse/coursera) lists none):
  - something under `https://www.coursera.org/api/` returning the user's certificate/accomplishment list, e.g. `onDemandCertificates*` / `userAccomplishments*`-style resources.
  - Public certificate page `https://www.coursera.org/account/accomplishments/verify/<VERIFY_CODE>` (**likely** to exist; verifier-facing, shows learner name, course, completion date). Hitting this page needs no session and its code is the natural `cert_id`. Because it is public, it proves page content but not that *this wallet's owner* holds it (anyone with the code can attest) → weaker; wallet-binding via `contextAddress` still works for whoever requests, so a copied code is a Sybil/replay hole. Prefer the logged-in list endpoint.
- Fields proving "completed": existence of a certificate entry is itself completion → leave `completion_keys: []` (our policy treats empty as 100%). Need title (`courseName`/`name`) and `verifyCode` for dedup. Discover real names by capturing the page in browser DevTools Network tab on `coursera.org/accomplishments` (action item, ~15 min).

## 3. Live attestor list

- SDK 5.8.3 `getAttestors()`: `GET https://api.reclaimprotocol.org/api/attestors` → `{data:[{address}]}` → mapped to `{id: address}` (confirmed, `dist/index.js` lines ~684, 1044; `BACKEND_BASE_URL = https://api.reclaimprotocol.org`, EU mirror `https://eu.api.reclaimprotocol.org` exists). `assertVerifiedProof` passes if any recovered signer is in the list.
- Live response today (confirmed, curl):
  - `0x1be31a94361a391bbafb2a4ccd704f57dc04d4bb`
  - `0x244897572368eadf65bfbc5aec98d8e5443a9072`
  - `0x15ea90114bffea9bb996e8775bf6ca0a338e97e5`
- `prover/src/sdk-verify.ts --online` already calls `getAttestors()` and lower-cases `a.id`; needs only outbound HTTPS to api.reclaimprotocol.org, no key. Nothing to change. Caveat: list is mutable; policy hash pins whichever you copy, so re-run before final proving. Which of the three signs a given proof depends on the session (Reclaim picks one) — a real proof is the only way to know; trusting all three is reasonable. Note `eu.` endpoint may return a different set (**not checked**).

## 4. Proposed `policy.json` diff (not applied)

```diff
-  "attestors": [],
+  "attestors": [
+    "0x1be31a94361a391bbafb2a4ccd704f57dc04d4bb",
+    "0x244897572368eadf65bfbc5aec98d8e5443a9072",
+    "0x15ea90114bffea9bb996e8775bf6ca0a338e97e5"
+  ],
   "platforms": [
     {
       "name": "udemy",
       "hosts": ["www.udemy.com"],
-      "path_prefixes": ["/api-2.0/"],
-      "title_keys": ["title", "course_title", "courseTitle", "Title"],
-      "completion_keys": ["completion_ratio", "completionRatio", "Completion Ratio"],
+      "path_prefixes": ["/api-2.0/users/me/subscribed-courses/"],
+      "title_keys": ["title"],
+      "completion_keys": ["completion_ratio"],
       "completion_is_fraction": false,
-      "cert_id_keys": ["certificate_id", "cert_id", "certificateId"]
+      "cert_id_keys": []
     },
```
Rationale: tightening the Udemy block to the one confirmed endpoint/variables shrinks what a malicious but attested proof (e.g. `purchase-history`, `Last Purchase` provider) could pass as a "course". Keep the old broader key lists if you expect a custom provider with other names. Coursera block: **leave as is** until a real endpoint is captured; then set `path_prefixes` to its path and `title_keys`/`cert_id_keys` to the captured variable names (`verifyCode` if exposed).

Also: policy attestor list and Udemy path are public inputs, so changing them changes the policy hash — regenerate `policy.test.json` unaffected (it trusts only the test attestor).

## Top gaps / next actions
1. Get Reclaim dev creds (APP_ID/SECRET) and generate one real Udemy Course Name proof; confirm signer ∈ the 3 addresses and `completion_ratio` scale.
2. Decide multi-course story: custom Udemy provider with `results[0..n]` or accept 1 proof per course after re-ordering.
3. Capture Coursera accomplishments request in DevTools; build custom provider; budget for Reclaim approval time (deadline risk → fall back to Udemy-only demo + Coursera as documented design).
