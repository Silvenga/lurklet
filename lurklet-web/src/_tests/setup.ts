import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterAll, afterEach, beforeAll } from "vitest";
import { MOCK_OPTIONS } from "./mocks/options";
import { server } from "./mocks/server";

beforeAll(() => server.listen(MOCK_OPTIONS));

afterEach(() => {
  cleanup();
  server.resetHandlers();
});

afterAll(() => server.close());
