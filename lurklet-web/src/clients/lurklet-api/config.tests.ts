// @vitest-environment node
/// <reference types="node" />

import { createServer } from "node:http";
import { http, HttpResponse } from "msw/http";
import { afterAll, expect, expectTypeOf, test, vi } from "vitest";
import { HEALTH_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { networkGuard } from "@/_tests/network-guard";
import type { Health, Problem } from "./generated/types.gen";

vi.stubGlobal("location", new URL("http://localhost:3000"));
const { getHealth } = await import("./generated/sdk.gen");

afterAll(() => vi.unstubAllGlobals());

test("When the API responds successfully then the generated client should return its typed data", async () => {
  const { data, response } = await getHealth();

  expect(data).toEqual({ health: "ok" });
  expectTypeOf(data).toEqualTypeOf<Health>();
  expect(response.headers.get("Cache-Control")).toBe("no-store");
});

test("When a request is made then it should use the current origin and default HTTP caching", async () => {
  const requests: Request[] = [];
  server.use(
    http.get("*/api/health", ({ request }) => {
      requests.push(request);
      return HttpResponse.json(HEALTH_RESPONSE);
    }),
  );

  await getHealth();

  expect(requests).toHaveLength(1);
  expect(requests[0]?.url).toBe(new URL("/api/health", location.origin).href);
  expect(requests[0]?.cache).toBe("default");
});

test("When the API returns Problem Details then the client should reject with that problem", async () => {
  const problem = {
    type: "about:blank",
    title: "Method Not Allowed",
    status: 405,
  } satisfies Problem;
  server.use(
    http.get("*/api/health", () =>
      HttpResponse.json(problem, {
        status: 405,
        headers: { "Content-Type": "application/problem+json" },
      }),
    ),
  );

  const result = getHealth();

  await expect(result).rejects.toEqual(problem);
});

test("When the API fails then the client should not automatically retry", async () => {
  let requests = 0;
  server.use(
    http.get("*/api/health", () => {
      requests++;
      return HttpResponse.json({ title: "Unavailable", status: 503 }, { status: 503 });
    }),
  );

  const result = getHealth();

  await expect(result).rejects.toMatchObject({ status: 503 });
  expect(requests).toBe(1);
});

test("When the caller aborts then the client should cancel the pending request", async () => {
  const started = Promise.withResolvers<void>();
  const pending = Promise.withResolvers<Response>();
  const controller = new AbortController();
  server.use(
    http.get("*/api/health", () => {
      started.resolve();
      return pending.promise;
    }),
  );

  const result = getHealth({ signal: controller.signal });
  await started.promise;
  controller.abort();
  pending.resolve(HttpResponse.json(HEALTH_RESPONSE));

  await expect(result).rejects.toMatchObject({ name: "AbortError" });
});

test.each(["/api", "/api/unhandled", "/unhandled"])(
  "When %s has no mock handler then its request should fail instead of reaching a server",
  async (path) => {
    let liveRequests = 0;
    const backend = createServer((_, response) => {
      liveRequests++;
      response.end("Unexpected live API request");
    });
    await new Promise<void>((resolve) => backend.listen(0, "127.0.0.1", resolve));
    const address = backend.address();
    if (!address || typeof address === "string") {
      throw new Error("The test API did not bind a TCP port");
    }
    const log = vi.spyOn(console, "error").mockImplementation(() => {});

    try {
      const result = fetch(new URL(path, `http://127.0.0.1:${address.port}`));

      await expect(result).rejects.toThrow();
      expect(log).toHaveBeenCalled();
      expect(liveRequests).toBe(0);
      expect(networkGuard.violations.splice(0)).toEqual([
        `Unhandled test request: http://127.0.0.1:${address.port}${path}`,
      ]);
    } finally {
      log.mockRestore();
      backend.closeAllConnections();
      await new Promise<void>((resolve, reject) => {
        backend.close((error) => (error ? reject(error) : resolve()));
      });
    }
  },
);
