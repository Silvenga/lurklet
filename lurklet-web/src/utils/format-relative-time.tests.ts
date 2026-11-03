// @vitest-environment node

import { expect, test } from "vitest";
import { formatRelativeTime } from "./format-relative-time";

test.each([
  [0, 0, null],
  [59, 0, null],
  [60, -1, "minute"],
  [119, -1, "minute"],
  [120, -2, "minute"],
  [3599, -59, "minute"],
  [3600, -1, "hour"],
  [7199, -1, "hour"],
  [86399, -23, "hour"],
  [86400, -1, "day"],
  [2591999, -29, "day"],
  [2592000, -1, "month"],
  [31536000, -1, "year"],
  [63072000, -2, "year"],
  [-59, 0, null],
  [-60, 1, "minute"],
  [-3600, 1, "hour"],
  [-86400, 1, "day"],
  [-2592000, 1, "month"],
  [-31536000, 1, "year"],
] as const)(
  "When a timestamp is %i seconds old then formatting should use the expected relative unit",
  (secondsAgo, value, unit) => {
    const now = Date.parse("2026-10-26T12:00:00Z");
    const expected =
      unit === null
        ? "just now"
        : new Intl.RelativeTimeFormat(undefined, { numeric: "always" }).format(value, unit);

    const result = formatRelativeTime(now - secondsAgo * 1000, now);

    expect(result).toBe(expected);
  },
);
