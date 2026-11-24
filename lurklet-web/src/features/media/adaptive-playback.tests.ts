import { expect, test, vi } from "vitest";
import { startAdaptivePlayback } from "./adaptive-playback";

const { attach, load } = vi.hoisted(() => ({ attach: vi.fn(), load: vi.fn() }));
vi.mock("shaka-player", () => ({
  default: {
    polyfill: { installAll() {} },
    Player: class {
      attach = attach;
      load = load;
      addEventListener() {}
      getNetworkingEngine() {
        return null;
      }
    },
  },
}));

test.each([
  [
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15",
    true,
  ],
  [
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1",
    true,
  ],
  ["Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36", false],
] as const)(
  "When HLS is selected with %s then explicit acceptance should be required only for Safari",
  async (userAgent, needsConsent) => {
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue(userAgent);
    load.mockClear();
    const video = document.createElement("video");
    const signal = new AbortController().signal;
    const url = "/api/media/v.redd.it/fixture/index.m3u8";

    await startAdaptivePlayback(video, "hls", url, false, signal, vi.fn());
    expect(load).toHaveBeenCalledTimes(needsConsent ? 0 : 1);
    if (needsConsent) {
      await startAdaptivePlayback(video, "hls", url, true, signal, vi.fn());
      expect(load).toHaveBeenCalledTimes(1);
    }
  },
);
