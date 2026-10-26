// @vitest-environment node

import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

const PROBE_TIMEOUT_MS = 2000;

test.each([
  ["mocked", 0, ""],
  ["https-mocked", 0, ""],
  ["abort", 0, ""],
  ["reuse", 0, ""],
  ["unhandled-api", 1, "Unhandled test request:"],
  ["unhandled-asset", 1, "Unhandled test request:"],
  ["passthrough", 1, "Unexpected real network connection:"],
  ["late", 1, "Unexpected real network connection:"],
] as const)(
  "When the network probe is %s then the process should exit with %i",
  { timeout: PROBE_TIMEOUT_MS + 500 },
  async (mode, code, message) => {
    const path = fileURLToPath(new URL("./test-network-probe.ts", import.meta.url));

    const result = await new Promise<{
      code: string | number | undefined;
      killed: boolean;
      stderr: string;
    }>((resolve) => {
      execFile(
        process.execPath,
        [path, mode],
        { timeout: PROBE_TIMEOUT_MS },
        (error, _stdout, stderr) => {
          resolve({ code: error?.code ?? 0, killed: error?.killed ?? false, stderr });
        },
      );
    });

    expect(result.killed, result.stderr).toBe(false);
    expect(result.code, result.stderr).toBe(code);
    if (message) expect(result.stderr).toContain(message);
    else expect(result.stderr).toBe("");
  },
);
