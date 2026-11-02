/**
 * zkTLS step without Reclaim's hosted app: create a wallet-bound claim of one
 * Udemy or Coursera course on an attestor we run ourselves (Reclaim's
 * open-source attestor-core), and save it in the same proof format the SP1
 * verifier already reads.
 *
 *   npm run attest -- --wallet 0xYourWallet [--platform udemy|coursera] [--start N]
 *                     [--curl ~/udemy.curl] [--attestor ws://localhost:8001/ws]
 *   npm run attest -- --pins     # print the provider hashes policy.json pins
 *
 * Udemy claims the most recently accessed course. Coursera claims enrollment
 * number N (--start, 0 to MAX_COURSERA_START).
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
import { hashProofClaimParams, isHttpProviderClaimParams } from "@reclaimprotocol/js-sdk";
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

// `limit=1` returns one membership and its one linked course, so a title cannot
// be paired with another course's score. `score` is a 0-1 fraction.
export const MAX_COURSERA_START = 4;
export function courseraParams(start: number): ProviderParams<"http"> {
  return {
    url: `https://www.coursera.org/api/memberships.v1?q=me&start=${start}&limit=1&includes=courseId&fields=grade,courses.v1(name)&showHidden=true&filter=current,preEnrolled,completed`,
    method: "GET",
    responseMatches: [
      { type: "regex", value: '"name":"(?<name>(?:[^"\\\\]|\\\\.)*)"' },
      { type: "regex", value: '"score":(?<score>[0-9.]+)' },
    ],
    responseRedactions: [
      { jsonPath: '$.linked["courses.v1"][0].name', regex: '"name":"(.*)"' },
      { jsonPath: "$.elements[0].grade.score", regex: '"score":(.*)' },
    ],
  };
}

function printPins(): void {
  const all = [UDEMY_PARAMS, ...Array.from({ length: MAX_COURSERA_START + 1 }, (_, i) => courseraParams(i))];
  for (const params of all) {
    if (isHttpProviderClaimParams(params)) console.log([hashProofClaimParams(params)].flat().join(" "), params.url.slice(8, 60));
  }
}

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
  const platform = arg("platform") ?? "udemy";
  if (platform !== "udemy" && platform !== "coursera") fail("invalid --platform (udemy|coursera)");
  const start = Number(arg("start") ?? "0");
  if (!Number.isInteger(start) || start < 0 || start > MAX_COURSERA_START) {
    fail(`invalid --start (0 to ${MAX_COURSERA_START})`);
  }
  const curlPath = arg("curl") ?? join(homedir(), `${platform}.curl`);
  const attestor = arg("attestor") ?? "ws://localhost:8001/ws";

  const { cookie, userAgent } = parseCurl(readFileSync(curlPath, "utf8"));
  const sessionCookie = platform === "udemy" ? "access_token=" : "CAUTH=";
  if (!cookie.includes(sessionCookie)) fail(`no ${sessionCookie} cookie found in ${curlPath}`);

  setCryptoImplementation(webcryptoCrypto);
  const nonce = randomBytes(8).toString("hex");
  const owner = Wallet.createRandom();

  const result = await createClaimOnAttestor({
    name: "http",
    params: platform === "udemy" ? UDEMY_PARAMS : courseraParams(start),
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
  const file = join(outDir, `${platform}-${Date.now()}.json`);
  writeFileSync(file, JSON.stringify([proof], null, 2) + "\n");
  console.log(`saved -> ${file}`);
  console.log("attestor:", signatures.attestorAddress);
  console.log("revealed fields:", JSON.parse(claim.context).extractedParameters);
}

// The attestor client keeps its WebSocket open in a shared pool; exit explicitly.
if (process.argv.includes("--pins")) printPins();
else await main();
process.exit(0);
