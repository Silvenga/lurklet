import { http, HttpResponse } from "msw/http";
import { HEALTH_RESPONSE } from "./fixtures";

export const handlers = [
  http.get("*/api/health", () =>
    HttpResponse.json(HEALTH_RESPONSE, { headers: { "Cache-Control": "no-store" } }),
  ),
];
