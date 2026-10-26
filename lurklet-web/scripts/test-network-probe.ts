import assert from "node:assert/strict";
import { passthrough } from "msw";
import { http, HttpResponse } from "msw/http";
import { setupServer } from "msw/node";
import { Agent, getGlobalDispatcher, setGlobalDispatcher } from "undici";
import { TEST_MOCK_OPTIONS } from "../src/_tests/mocks/test-options.ts";
import { networkGuard } from "../src/_tests/network-guard.ts";

const mode = process.argv[2];
const previousDispatcher = getGlobalDispatcher();
const dispatcher = new Agent();
const started = Promise.withResolvers<void>();
const pending = Promise.withResolvers<Response>();
const server = setupServer(
  http.get("*/ok", () => HttpResponse.json({ ok: true })),
  http.get("*/pending", () => {
    started.resolve();
    return pending.promise;
  }),
  http.get("*/passthrough", () => passthrough()),
);

networkGuard.phase = "mocking";
setGlobalDispatcher(dispatcher);
server.listen(TEST_MOCK_OPTIONS);

try {
  if (mode === "abort") {
    const controller = new AbortController();
    const response = fetch("http://127.0.0.1:65534/pending", { signal: controller.signal });
    await started.promise;
    controller.abort();
    pending.resolve(HttpResponse.json({ ok: true }));
    await assert.rejects(response, { name: "AbortError" });
  } else if (mode === "mocked" || mode === "https-mocked" || mode === "late" || mode === "reuse") {
    const protocol = mode === "https-mocked" ? "https" : "http";
    assert.deepEqual(await (await fetch(`${protocol}://127.0.0.1:65534/ok`)).json(), { ok: true });
  } else if (mode === "unhandled-api" || mode === "unhandled-asset" || mode === "passthrough") {
    const path =
      mode === "unhandled-api"
        ? "/api/unhandled"
        : mode === "unhandled-asset"
          ? "/asset.png"
          : "/passthrough";
    await fetch(`http://127.0.0.1:65534${path}`).catch(() => {});
  } else {
    throw new Error(`Unknown network probe: ${mode}`);
  }
} finally {
  await dispatcher.destroy();
  networkGuard.phase = "closed";
  server.close();
  setGlobalDispatcher(previousDispatcher);
}

if (mode === "late") {
  await fetch("http://127.0.0.1:65534/late").catch(() => {});
}

if (mode === "reuse") {
  const nextDispatcher = new Agent();
  networkGuard.phase = "mocking";
  setGlobalDispatcher(nextDispatcher);
  server.listen(TEST_MOCK_OPTIONS);
  try {
    assert.deepEqual(await (await fetch("http://127.0.0.1:65534/ok")).json(), { ok: true });
  } finally {
    await nextDispatcher.destroy();
    networkGuard.phase = "closed";
    server.close();
    setGlobalDispatcher(previousDispatcher);
  }
}
