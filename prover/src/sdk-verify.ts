/**
 * Cross-checks proofs with the official Reclaim SDK (independent of our Rust code).
 *
 *   npm run sdk-verify -- ../proofs/udemy-123.json [--policy ../policy.json] [--online]
 *
 * Prints each proof's recovered attestor and revealed fields. The verdict is the
 * SDK's own `assertVerifiedProof` against a trusted attestor set:
 *   --policy <file>  trust that policy's `attestors` (offline)
 *   --online         trust Reclaim's live attestor list (fetched)
 * Use the printed attestor, URL and provider hash(es) to fill in policy.json.
 *
 * The SDK only checks "signed by a trusted attestor". Wallet binding, context
 * tag, endpoint pinning and freshness are ours (crates/core), so the SDK
 * accepting a proof does not mean our verifier will.
 *
 * Exit code 1 if any proof is rejected.
 */
import { readFileSync } from "node:fs";
import {
  assertVerifiedProof,
  getAttestors,
  getIdentifierFromClaimInfo,
  hashProofClaimParams,
  isHttpProviderClaimParams,
  recoverSignersOfSignedClaim,
  type Proof,
  type ProviderClaimData,
  type WitnessData,
} from "@reclaimprotocol/js-sdk";
import { getBytes } from "ethers";

type StoredProof = Pick<Proof, "identifier" | "claimData" | "signatures">;

function isRecord(x: unknown): x is Record<string, unknown> {
  return typeof x === "object" && x !== null && !Array.isArray(x);
}

function isClaimData(x: unknown): x is ProviderClaimData {
  return (
    isRecord(x) &&
    typeof x.provider === "string" &&
    typeof x.parameters === "string" &&
    typeof x.owner === "string" &&
    typeof x.timestampS === "number" &&
    typeof x.context === "string" &&
    typeof x.identifier === "string" &&
    typeof x.epoch === "number"
  );
}

function isStoredProof(x: unknown): x is StoredProof {
  return (
    isRecord(x) &&
    typeof x.identifier === "string" &&
    isClaimData(x.claimData) &&
    Array.isArray(x.signatures) &&
    x.signatures.every((s) => typeof s === "string")
  );
}

/** Accepts a single proof, an array of proofs, or `{ proofs: [...] }` (program input). */
function readProofs(file: string): StoredProof[] {
  const raw: unknown = JSON.parse(readFileSync(file, "utf8"));
  const list: unknown[] = Array.isArray(raw)
    ? raw
    : isRecord(raw) && Array.isArray(raw.proofs)
      ? raw.proofs
      : [raw];
  return list.map((p, i) => {
    if (!isStoredProof(p)) throw new Error(`${file}[${i}]: not a Reclaim proof`);
    return p;
  });
}

function readPolicyAttestors(file: string): WitnessData[] {
  const raw: unknown = JSON.parse(readFileSync(file, "utf8"));
  if (!isRecord(raw) || !Array.isArray(raw.attestors)) {
    throw new Error(`${file}: no "attestors" array`);
  }
  return raw.attestors.map((a) => {
    if (typeof a !== "string") throw new Error(`${file}: attestor ${String(a)} is not a string`);
    return { id: a, url: "" };
  });
}

function parseJsonRecord(s: string): Record<string, unknown> {
  const v: unknown = JSON.parse(s || "{}");
  return isRecord(v) ? v : {};
}

const argv = process.argv.slice(2);
const files: string[] = [];
let policyFile: string | undefined;
let online = false;
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (a === "--online") online = true;
  else if (a === "--policy") policyFile = argv[++i];
  else if (a !== undefined && !a.startsWith("--")) files.push(a);
  else {
    console.error(`unknown argument: ${String(a)}`);
    process.exit(2);
  }
}
if (files.length === 0) {
  console.error("usage: sdk-verify <proof.json>... [--policy <policy.json>] [--online]");
  process.exit(2);
}

const trusted: WitnessData[] = [
  ...(policyFile ? readPolicyAttestors(policyFile) : []),
  ...(online ? await getAttestors() : []),
];
const haveTrustSet = policyFile !== undefined || online;
let rejected = 0;

for (const f of files) {
  for (const p of readProofs(f)) {
    const signers = recoverSignersOfSignedClaim({
      claim: p.claimData,
      signatures: p.signatures.map((s) => getBytes(s)),
    });
    const params = parseJsonRecord(p.claimData.parameters);
    const ctx = parseJsonRecord(p.claimData.context);
    const recomputed = getIdentifierFromClaimInfo(p.claimData);
    let verdict = "no trust set (pass --policy or --online)";
    if (haveTrustSet) {
      try {
        await assertVerifiedProof({ ...p, witnesses: [], extractedParameterValues: undefined }, trusted);
        verdict = "ACCEPT";
      } catch (e) {
        verdict = `REJECT (${e instanceof Error ? e.message : String(e)})`;
        rejected++;
      }
    }
    console.log(`\n${f}`);
    console.log("  sdk verdict", verdict);
    console.log("  url        ", params.url);
    const ph = isHttpProviderClaimParams(params) ? hashProofClaimParams(params) : "n/a (not an HTTP claim)";
    console.log("  provider # ", (Array.isArray(ph) ? ph : [ph]).join(", "));
    console.log("  signers    ", signers.join(", "));
    console.log(
      "  identifier ",
      recomputed === p.claimData.identifier && recomputed === p.identifier
        ? "matches SDK recomputation"
        : `MISMATCH (stored ${p.identifier}, recomputed ${recomputed})`,
    );
    console.log("  wallet     ", ctx.contextAddress, " message:", ctx.contextMessage);
    console.log("  fields     ", ctx.extractedParameters);
    console.log("  attested at", new Date(p.claimData.timestampS * 1000).toISOString());
  }
}

if (rejected > 0) process.exit(1);
