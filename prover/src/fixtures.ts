/**
 * Generates deterministic TEST fixtures in Reclaim's exact proof format.
 *
 * Why: real Reclaim proofs need a logged-in Udemy/Coursera account and a phone
 * or browser extension, which CI and reviewers don't have. These fixtures are
 * signed with a throwaway "test attestor" key, using the official SDK's own
 * `createSignDataForClaim` / `getIdentifierFromClaimInfo`, so the signing format
 * is byte-for-byte what real attestors sign. The test policy trusts ONLY the
 * test attestor; the real policy trusts ONLY Reclaim's attestors, so fixtures
 * can never pass under the real policy.
 *
 * Output: ../fixtures/{policy.test.json, input.test.json, negative/*.json}
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Wallet, keccak256, toUtf8Bytes } from "ethers";
import {
  createSignDataForClaim,
  getIdentifierFromClaimInfo,
  hashProofClaimParams,
  type HttpProviderClaimParams,
  type Proof,
  type ProviderClaimData,
} from "@reclaimprotocol/js-sdk";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "fixtures");
mkdirSync(join(root, "negative"), { recursive: true });

// Deterministic throwaway keys. NEVER use these for anything real.
const key = (label: string): string => keccak256(toUtf8Bytes(`eulerphi-zkcert-test:${label}`));
const testAttestor = new Wallet(key("attestor"));
const rogueAttestor = new Wallet(key("rogue-attestor"));
const sessionOwner = new Wallet(key("reclaim-session-owner"));
const userWallet = new Wallet(key("user-wallet")).address;
const otherWallet = new Wallet(key("someone-else")).address;

const CONTEXT_TAG = "eulerphi-zkcert:v1";
const T0 = Date.UTC(2026, 9, 28, 6, 0, 0) / 1000; // 2026-10-28 06:00 UTC
const EVALUATED_AT = T0 + 3600;

type Platform = "udemy" | "coursera";

const ENDPOINT: Record<Platform, string> = {
  udemy:
    "https://www.udemy.com/api-2.0/users/me/subscribed-courses/?fields[course]=title,completion_ratio",
  coursera: "https://www.coursera.org/api/memberships.v1?q=me&includes=courseId,certificate",
};

interface Spec {
  name: string;
  platform: Platform;
  fields: Record<string, string>;
  wallet?: string;
  tag?: string;
  ts?: number;
  url?: string;
  signer?: Wallet;
  /** Override the jsonPath of every redaction (simulates a claimant-written extraction spec). */
  redactionPath?: (field: string) => string;
  /** Claimant-supplied template inputs (never attested). */
  paramValues?: Record<string, string>;
  /** Override what the attestor extracted (defaults to `fields`). */
  extracted?: Record<string, string>;
}

// Reclaim proof shape as the SDK returns it, minus the fields we don't emit.
type FixtureProof = Pick<Proof, "identifier" | "claimData" | "signatures" | "witnesses"> & {
  publicData: null;
};

function makeProof(s: Spec): FixtureProof {
  const parameters = JSON.stringify({
    body: "",
    method: "GET",
    paramValues: s.paramValues ?? {},
    responseMatches: Object.keys(s.fields).map((k) => ({
      type: "regex",
      value: `"${k}":"?(?<${k}>[^",}]+)`,
    })),
    responseRedactions: Object.keys(s.fields).map((k) => ({ jsonPath: s.redactionPath?.(k) ?? `$..${k}` })),
    url: s.url ?? ENDPOINT[s.platform],
  });
  const context = JSON.stringify({
    contextAddress: s.wallet ?? userWallet,
    contextMessage: `${s.tag ?? CONTEXT_TAG}:nonce-${s.name}`,
    extractedParameters: s.extracted ?? s.fields,
    providerHash: keccak256(toUtf8Bytes(`provider:${s.platform}`)),
  });
  const claim: ProviderClaimData = {
    provider: "http",
    parameters,
    owner: sessionOwner.address.toLowerCase(),
    timestampS: s.ts ?? T0,
    context,
    identifier: "",
    epoch: 1,
  };
  claim.identifier = getIdentifierFromClaimInfo(claim);
  const signer = s.signer ?? testAttestor;
  const signature = signer.signMessageSync(createSignDataForClaim(claim));
  return {
    identifier: claim.identifier,
    claimData: claim,
    signatures: [signature],
    witnesses: [{ id: signer.address.toLowerCase(), url: "wss://test-attestor.invalid/ws" }],
    publicData: null,
  };
}

const udemy = (title: string, completion: string, certId: string): Record<string, string> => ({
  title,
  completion_ratio: completion,
  certificate_id: certId,
});
const coursera = (courseName: string, verifyCode: string): Record<string, string> => ({ courseName, verifyCode });

// The happy-path set: 3 distinct crypto credentials + noise.
const valid: Spec[] = [
  { name: "udemy-solidity", platform: "udemy", fields: udemy("Ethereum and Solidity: The Complete Developer's Guide", "100", "UC-11111111") },
  { name: "coursera-blockchain", platform: "coursera", fields: coursera("Blockchain Basics", "ABCD1234EFGH") },
  { name: "udemy-zk", platform: "udemy", fields: udemy("Zero-Knowledge Proofs in Rust", "100", "UC-33333333") },
  { name: "udemy-web3-incomplete", platform: "udemy", fields: udemy("Web3 dApp Bootcamp", "45", "") },
  { name: "coursera-ml", platform: "coursera", fields: coursera("Machine Learning Specialization", "ML99887766") },
  { name: "udemy-solidity-duplicate", platform: "udemy", fields: udemy("Ethereum and Solidity: The Complete Developer's Guide", "100", "UC-11111111") },
];

// Each must make the SP1 program abort.
const negative: Spec[] = [
  { name: "other-wallet", platform: "udemy", wallet: otherWallet, fields: udemy("Solidity Security", "100", "UC-4") },
  { name: "rogue-attestor", platform: "udemy", signer: rogueAttestor, fields: udemy("Solidity Security", "100", "UC-5") },
  { name: "look-alike-host", platform: "udemy", url: "https://www.udemy.com.evil.example/api-2.0/users/me/", fields: udemy("Solidity Security", "100", "UC-6") },
  { name: "wrong-endpoint", platform: "udemy", url: "https://www.udemy.com/course/some-public-page/", fields: udemy("Solidity Security", "100", "UC-7") },
  { name: "foreign-app-tag", platform: "udemy", tag: "some-other-app:v1", fields: udemy("Solidity Security", "100", "UC-8") },
  { name: "stale", platform: "udemy", ts: T0 - 400 * 86400, fields: udemy("Solidity Security", "100", "UC-9") },
  // Same endpoint, but the claimant picked which array element each field comes from.
  { name: "unpinned-provider", platform: "udemy", redactionPath: (k) => `$.results[7].${k}`, fields: udemy("Solidity Security", "100", "UC-10") },
  // Title only in claimant-supplied paramValues, not extracted by the attestor.
  {
    name: "param-values-title",
    platform: "udemy",
    fields: udemy("Solidity Security", "100", "UC-11"),
    paramValues: { title: "Blockchain Mastery" },
    extracted: { completion_ratio: "100", certificate_id: "UC-11" },
  },
  // Tag is a prefix of ours but a different app version.
  { name: "tag-prefix", platform: "udemy", tag: `${CONTEXT_TAG}0`, fields: udemy("Solidity Security", "100", "UC-12") },
];

// Provider hashes (computed by the SDK itself) of the request specs the valid set uses.
const providerHashes = (platform: Platform): string[] => {
  const hashes = valid
    .filter((v) => v.platform === platform)
    .flatMap((v) => {
      const params = JSON.parse(makeProof(v).claimData.parameters) as HttpProviderClaimParams;
      const h = hashProofClaimParams(params);
      return Array.isArray(h) ? h : [h];
    });
  return [...new Set(hashes)].sort();
};

const policy = {
  version: 1,
  context_tag: CONTEXT_TAG,
  attestors: [testAttestor.address.toLowerCase()],
  platforms: [
    {
      name: "udemy",
      hosts: ["www.udemy.com"],
      path_prefixes: ["/api-2.0/users/me/"],
      title_keys: ["title", "course_title"],
      completion_keys: ["completion_ratio", "completionRatio"],
      completion_is_fraction: false,
      cert_id_keys: ["certificate_id", "cert_id"],
      provider_hashes: providerHashes("udemy"),
    },
    {
      name: "coursera",
      hosts: ["www.coursera.org"],
      path_prefixes: ["/api/"],
      title_keys: ["courseName", "title", "name"],
      completion_keys: [],
      completion_is_fraction: false,
      cert_id_keys: ["verifyCode", "certificate_id"],
      provider_hashes: providerHashes("coursera"),
    },
  ],
  industry: {
    id: "crypto",
    keywords: [
      "blockchain", "ethereum", "solidity", "smart contract", "smart contracts", "web3", "defi",
      "cryptography", "zero-knowledge", "zero knowledge", "zk*", "bitcoin", "solana",
    ],
  },
  min_count: 2,
  min_completion_pct: 100,
  max_age_secs: 180 * 86400,
};

const write = (rel: string, obj: unknown): void =>
  writeFileSync(join(root, rel), JSON.stringify(obj, null, 2) + "\n");

write("policy.test.json", policy);
write("input.test.json", {
  proofs: valid.map(makeProof),
  policy,
  wallet: userWallet,
  evaluated_at: EVALUATED_AT,
});
for (const n of negative) write(join("negative", `${n.name}.json`), makeProof(n));
write("meta.json", {
  note: "TEST ONLY. Throwaway keys derived from public labels. Never trust these addresses.",
  test_attestor: testAttestor.address.toLowerCase(),
  rogue_attestor: rogueAttestor.address.toLowerCase(),
  user_wallet: userWallet,
  other_wallet: otherWallet,
  evaluated_at: EVALUATED_AT,
  expected: { qualified_count: 3, passed: true },
});
console.log(`wrote fixtures to ${root}`);
