import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { Agent, getGlobalDispatcher, setGlobalDispatcher } from "undici";
import { afterAll, afterEach, beforeAll, beforeEach, expect } from "vitest";
import { server } from "./mocks/server";
import { TEST_MOCK_OPTIONS } from "./mocks/test-options";
import { networkGuard } from "./network-guard";

const previousDispatcher = getGlobalDispatcher();
let dispatcher: Agent;

beforeAll(() => {
  expect(networkGuard.violations).toEqual([]);
  networkGuard.phase = "mocking";
  server.listen(TEST_MOCK_OPTIONS);
});

beforeEach(() => {
  dispatcher = new Agent();
  setGlobalDispatcher(dispatcher);
});

afterEach(async () => {
  cleanup();
  await dispatcher.destroy();
  server.resetHandlers();
  expect(networkGuard.violations.splice(0)).toEqual([]);
});

afterAll(() => {
  networkGuard.phase = "closed";
  server.close();
  setGlobalDispatcher(previousDispatcher);
});
