/**
 * zkTLS step: get a Reclaim proof of a Udemy or Coursera credential, bound to
 * the user's wallet, and save it to ../proofs/.
 *
 *   npm run request -- --platform udemy --wallet 0xYourWallet
 *
 * Prints a link + QR code. The user opens it (Reclaim app or browser extension),
 * logs into Udemy/Coursera, and the attestor observes the TLS session. Only the
 * fields the provider extracts (course title, completion, certificate id) are
 * revealed; cookies and account data stay private.
 *
 * Env (.env): RECLAIM_APP_ID, RECLAIM_APP_SECRET, UDEMY_PROVIDER_ID, COURSERA_PROVIDER_ID
 */
import "dotenv/config";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { randomBytes } from "node:crypto";
import { isAddress } from "ethers";
import qrcode from "qrcode-terminal";
import { ReclaimProofRequest, type Proof } from "@reclaimprotocol/js-sdk";

// Must equal `context_tag` in policy.json.
const CONTEXT_TAG = "eulerphi-zkcert:v1";

const ENV_HELP = "copy prover/.env.example to prover/.env and fill it in";

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i > -1 ? process.argv[i + 1] : undefined;
}
function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

const platform = arg("platform");
if (platform !== "udemy" && platform !== "coursera") fail("missing or invalid --platform (udemy|coursera)");
const wallet = arg("wallet");
if (wallet === undefined || !isAddress(wallet)) fail("missing or invalid --wallet (0x-prefixed address)");

// Check every required key up front so the user fixes .env in one go.
const providerKey = `${platform.toUpperCase()}_PROVIDER_ID`;
const required = ["RECLAIM_APP_ID", "RECLAIM_APP_SECRET", providerKey];
const missing = required.filter((k) => !process.env[k]?.trim());
if (missing.length > 0) fail(`missing in .env: ${missing.join(", ")} (${ENV_HELP})`);
const env = (k: string): string => process.env[k]?.trim() ?? "";
const appId = env("RECLAIM_APP_ID");
const appSecret = env("RECLAIM_APP_SECRET");
const providerId = env(providerKey);

const outDir = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "proofs");
mkdirSync(outDir, { recursive: true });

const request = await ReclaimProofRequest.init(appId, appSecret, providerId);

// Wallet binding + domain separation. The context is hashed into the claim
// identifier the attestor signs, so it cannot be changed after the fact.
const nonce = randomBytes(8).toString("hex");
request.setContext(wallet, `${CONTEXT_TAG}:${nonce}`);

const url = await request.getRequestUrl();
console.log(`\nOpen this link (or scan) and log into ${platform}:\n${url}\n`);
qrcode.generate(url, { small: true });

await request.startSession({
  // The SDK's default flow already verifies attestor signatures and the
  // provider hash before calling this. SP1 re-checks everything independently.
  onSuccess: (proofOrProofs: Proof | Proof[]): void => {
    const proofs = Array.isArray(proofOrProofs) ? proofOrProofs : [proofOrProofs];
    if (proofs.length === 0) {
      console.error("No proofs returned (callback URL mode?).");
      process.exit(1);
    }
    const file = join(outDir, `${platform}-${Date.now()}.json`);
    writeFileSync(file, JSON.stringify(proofs, null, 2) + "\n");
    console.log(`saved ${proofs.length} proof(s) -> ${file}`);
    for (const p of proofs) {
      const ctx: unknown = JSON.parse(p.claimData.context || "{}");
      const fields = typeof ctx === "object" && ctx !== null && "extractedParameters" in ctx
        ? ctx.extractedParameters
        : undefined;
      console.log("revealed fields:", fields);
    }
    process.exit(0);
  },
  onError: (err: Error): void => {
    console.error("proof generation failed:", err.message);
    process.exit(1);
  },
});
