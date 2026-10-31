/**
 * zkTLS step without Reclaim's hosted app: create a wallet-bound claim of the
 * user's most recently accessed Udemy course on an attestor we run ourselves
 * (Reclaim's open-source attestor-core), and save it in the same proof format
 * the SP1 verifier already reads.
 *
 *   npm run attest -- --wallet 0xYourWallet [--curl ~/udemy.curl] [--attestor ws://localhost:8001/ws]
 *
 * --curl is a request copied from the browser ("Copy as cURL"); only its cookie
 * and User-Agent are used, as secret parameters the attestor never sees.
 */
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { randomBytes } from "node:crypto";
import { Wallet, hexlify, isAddress } from "ethers";
import { createClaimOnAttestor, getIdentifierFromClaimInfo, type ProviderParams } from "@reclaimprotocol/attestor-core";
import { setCryptoImplementation } from "@reclaimprotocol/tls";
import { webcryptoCrypto } from "@reclaimprotocol/tls/webcrypto";

// Must equal `context_tag` in policy.json.
const CONTEXT_TAG = "eulerphi-zkcert:v1";

// The provider spec. Every byte of it is hashed into the claim's provider hash,
// which policy.json pins, so changing anything here needs a new pin.
export const UDEMY_PARAMS: ProviderParams<"http"> = {
  url: "https://www.udemy.com/api-2.0/users/me/subscribed-courses/?ordering=-last_accessed&page_size=1&fields[course]=title,completion_ratio",
  method: "GET",
  responseMatches: [
    { type: "regex", value: '"title":"(?<title>(?:[^"\\\\]|\\\\.)*)"' },
    { type: "regex", value: '"completion_ratio":(?<completion_ratio>\\d+)' },
  ],
  responseRedactions: [
    { jsonPath: "$.results[0].title", regex: '"title":"(.*)"' },
    { jsonPath: "$.results[0].completion_ratio", regex: '"completion_ratio":(.*)' },
  ],
};

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i > -1 ? process.argv[i + 1] : undefined;
}
function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

/** Pull the cookie and User-Agent out of a "Copy as cURL (bash)" file. */
export function parseCurl(text: string): { cookie: string; userAgent: string } {
  const quoted = [...text.matchAll(/(-H|--header|-b|--cookie)\s+(?:'([^']*)'|"([^"]*)")/g)];
  let cookie = "";
  let userAgent = "";
  for (const [, flag, single, double] of quoted) {
    const value = single ?? double ?? "";
    if (flag === "-b" || flag === "--cookie") cookie = value;
    else if (/^cookie:/i.test(value)) cookie = value.replace(/^cookie:\s*/i, "");
    else if (/^user-agent:/i.test(value)) userAgent = value.replace(/^user-agent:\s*/i, "");
  }
  return { cookie, userAgent };
}

async function main(): Promise<void> {
  const wallet = arg("wallet");
  if (wallet === undefined || !isAddress(wallet)) fail("missing or invalid --wallet (0x-prefixed address)");
  const curlPath = arg("curl") ?? join(homedir(), "udemy.curl");
  const attestor = arg("attestor") ?? "ws://localhost:8001/ws";

  const { cookie, userAgent } = parseCurl(readFileSync(curlPath, "utf8"));
  if (!cookie.includes("access_token=")) fail(`no Udemy access_token cookie found in ${curlPath}`);

  setCryptoImplementation(webcryptoCrypto);
  const nonce = randomBytes(8).toString("hex");
  const owner = Wallet.createRandom();

  const result = await createClaimOnAttestor({
    name: "http",
    params: UDEMY_PARAMS,
    secretParams: {
      cookieStr: cookie,
      headers: { "User-Agent": userAgent, Accept: "application/json, text/plain, */*" },
    },
    context: { contextAddress: wallet, contextMessage: `${CONTEXT_TAG}:${nonce}` },
    ownerPrivateKey: owner.privateKey,
    client: { url: attestor },
  });
  if (result.error) fail(`attestor rejected the claim: ${result.error.message}`);
  if (!result.claim || !result.signatures) fail("attestor returned no claim");

  const { claim, signatures } = result;
  const identifier = getIdentifierFromClaimInfo(claim);
  const proof = {
    identifier,
    claimData: { ...claim, identifier },
    signatures: [hexlify(signatures.claimSignature)],
    witnesses: [{ id: signatures.attestorAddress, url: attestor }],
  };

  const outDir = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "proofs");
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, `udemy-${Date.now()}.json`);
  writeFileSync(file, JSON.stringify([proof], null, 2) + "\n");
  console.log(`saved -> ${file}`);
  console.log("attestor:", signatures.attestorAddress);
  console.log("revealed fields:", JSON.parse(claim.context).extractedParameters);
}

await main();
