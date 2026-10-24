import type { SharedOptions } from "msw";
import { HttpResponse } from "msw/http";

export const MOCK_OPTIONS: SharedOptions = {
  onUnhandledFrame({ frame, defaults }) {
    if (frame.protocol === "http") {
      const { request } = frame.data as { request: Request };
      const { pathname } = new URL(request.url);

      if (pathname === "/api" || pathname.startsWith("/api/")) {
        defaults.error();
        throw HttpResponse.error();
      }
    }
  },
};
