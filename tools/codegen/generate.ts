// Runs in its own package: Hey API needs the registered TS 6 compiler API.
import { resolve } from "node:path";

const { createClient } = await import("@hey-api/openapi-ts");
const output = process.argv[2];
if (!output) throw new Error("choose a generated-contract directory");
await createClient({
  input: resolve("contracts/openapi.json"),
  output: resolve(output),
  plugins: ["@hey-api/typescript"],
  logs: { level: "silent" },
});
