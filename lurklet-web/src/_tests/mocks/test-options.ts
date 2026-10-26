import type { SharedOptions } from "msw";
import { HttpResponse } from "msw/http";
import { networkGuard } from "../network-guard.ts";

export const TEST_MOCK_OPTIONS: SharedOptions = {
  onUnhandledFrame({ frame, defaults }) {
    const target =
      frame.protocol === "http" ? (frame.data as { request: Request }).request.url : frame.protocol;
    networkGuard.violations.push(`Unhandled test request: ${target}`);
    defaults.error();
    throw HttpResponse.error();
  },
};
