import { expect, onTestFinished, test, vi } from "vitest";
import { withResponseAge } from "./with-response-age";

test.each([
  [null, undefined],
  ["", undefined],
  ["0", 0],
  ["3060", 3060],
  ["-1", undefined],
  ["1.5", undefined],
  ["1e3", undefined],
  ["12 seconds", undefined],
  ["1, 2", undefined],
  ["9007199254740991", undefined],
] as const)(
  "When Age is %s then the response timestamp should use only valid whole seconds",
  async (age, seconds) => {
    const now = Date.parse("2026-11-02T12:00:00Z");
    const clock = vi.spyOn(Date, "now").mockReturnValue(now);
    onTestFinished(() => clock.mockRestore());
    const data = { title: "Post" };
    const response = new Response(null, {
      headers: {
        ...(age === null ? {} : { Age: age }),
        Date: "Mon, 02 Nov 2026 10:00:00 GMT",
        "X-Lurklet-Cache": "stale",
      },
    });

    const result = await withResponseAge(Promise.resolve({ data, response }));

    expect(result.data).toBe(data);
    expect(result.fetchedAt).toBe(seconds === undefined ? undefined : now - seconds * 1000);
  },
);
