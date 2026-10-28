/**
 * Cross-checks proofs with the official Reclaim SDK (independent of our Rust code).
 *
 *   npm run sdk-verify -- ../proofs/udemy-123.json [--online]
 *
 * Prints each proof's recovered attestor and revealed fields. With --online it
 * also fetches Reclaim's live attestor list and says whether the signer is on it.
 * Use the printed attestor + URL to fill in policy.json.
 */
import { readFileSync } from "node:fs";
import { getAttestors, recoverSignersOfSignedClaim } from "@reclaimprotocol/js-sdk";
import { getBytes } from "ethers";

const files = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const online = process.argv.includes("--online");
const live = online ? new Set((await getAttestors()).map((a) => a.id.toLowerCase())) : null;

for (const f of files) {
  const raw = JSON.parse(readFileSync(f, "utf8"));
  const proofs = Array.isArray(raw) ? raw : raw.proofs ?? [raw];
  for (const p of proofs) {
    const signers = recoverSignersOfSignedClaim({
      claim: p.claimData,
      signatures: p.signatures.map((s: string) => getBytes(s)),
    });
    const params = JSON.parse(p.claimData.parameters);
    const ctx = JSON.parse(p.claimData.context || "{}");
    console.log(`\n${f}`);
    console.log("  url        ", params.url);
    console.log("  signers    ", signers.join(", "));
    if (live) console.log("  on live attestor list:", signers.some((s) => live.has(s)));
    console.log("  wallet     ", ctx.contextAddress, " message:", ctx.contextMessage);
    console.log("  fields     ", ctx.extractedParameters);
    console.log("  attested at", new Date(p.claimData.timestampS * 1000).toISOString());
  }
}
