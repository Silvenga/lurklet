import Bowser from "bowser";
import shaka from "shaka-player";

shaka.polyfill.installAll();

export async function startAdaptivePlayback(
  video: HTMLVideoElement,
  format: "dash" | "hls",
  url: string,
  accepted: boolean,
  signal: AbortSignal,
  update: (status: "consent" | "failed" | "ready") => void,
) {
  if (
    format === "hls" &&
    !accepted &&
    Bowser.getParser(navigator.userAgent).getBrowserName() === "Safari"
  ) {
    update("consent");
    return;
  }
  const player = new shaka.Player();
  signal.addEventListener(
    "abort",
    () => {
      void player.destroy();
    },
    { once: true },
  );
  player.addEventListener("error", (event) => {
    if (
      !signal.aborted &&
      (event as CustomEvent<shaka.util.Error>).detail.severity ===
        shaka.util.Error.Severity.CRITICAL
    )
      update("failed");
  });
  await player.attach(video);
  if (signal.aborted) return;
  const manifest = new URL(url, location.href);
  player.getNetworkingEngine()?.registerRequestFilter((_type, request) => {
    for (const uri of request.uris) {
      const target = new URL(uri, manifest);
      if (target.origin !== manifest.origin || !target.pathname.startsWith("/api/media/"))
        throw new Error("Unproxied media reference");
    }
  });
  await player.load(
    url,
    undefined,
    format === "dash" ? "application/dash+xml" : "application/vnd.apple.mpegurl",
  );
  if (!signal.aborted) update("ready");
}
