import { useEffect, useRef, useState } from "react";
import type { VideoSource } from "@/clients/lurklet-api/generated/types.gen";
import { Alert, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { buildMediaUrl } from "../build-media-url";
import { Mp4Video } from "./Mp4Video";

type Props = {
  sources: VideoSource[];
  posterUrl: string | null;
  dash: string | undefined;
  hls: string | undefined;
};

export function AdaptiveVideo({ sources, posterUrl, dash, hls }: Props) {
  const video = useRef<HTMLVideoElement>(null);
  const [format, setFormat] = useState<VideoSource["format"]>(dash ? "dash" : "hls");
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<"loading" | "consent" | "failed" | "ready">("loading");
  useEffect(() => {
    const element = video.current;
    const url = format === "dash" ? dash : hls;
    if (!element || !url || format === "mp4") return;
    const controller = new AbortController();
    void import("../adaptive-playback")
      .then(async ({ startAdaptivePlayback }) => {
        if (controller.signal.aborted) return;
        await startAdaptivePlayback(
          element,
          format,
          url,
          attempt > 0,
          controller.signal,
          setStatus,
        );
      })
      .catch(() => {
        if (!controller.signal.aborted) setStatus("failed");
      });
    return () => controller.abort();
  }, [format, attempt, dash, hls]);

  function select(next: VideoSource["format"]) {
    setStatus("loading");
    setFormat(next);
    setAttempt((n) => n + 1);
  }
  if (format === "mp4") return <Mp4Video sources={sources} posterUrl={posterUrl} />;
  return (
    <div className="flex flex-col gap-2">
      {/* oxlint-disable-next-line jsx-a11y/media-has-caption - Adaptive captions come from the manifest. */}
      <video
        key={`${format}:${attempt}`}
        ref={video}
        controls={status === "ready"}
        playsInline
        preload="none"
        poster={buildMediaUrl(posterUrl)}
        aria-label="Video"
        className="bg-muted block h-auto max-h-[90dvh] w-full rounded-lg"
      />
      {(status === "failed" || status === "consent") && (
        <Alert>
          <AlertTitle>
            {status === "consent"
              ? "HLS playback is available"
              : `${format.toUpperCase()} playback failed`}
          </AlertTitle>
          <div className="flex flex-wrap gap-2">
            {dash && (
              <Button onPress={() => select("dash")}>
                {format === "dash" ? "Retry DASH" : "Play DASH"}
              </Button>
            )}
            {hls && (
              <Button onPress={() => select("hls")}>
                {format === "hls" && status === "failed" ? "Retry HLS" : "Play HLS"}
              </Button>
            )}
            {sources.some((s) => s.format === "mp4" && buildMediaUrl(s.url)) && (
              <Button onPress={() => select("mp4")}>Play MP4</Button>
            )}
          </div>
        </Alert>
      )}
    </div>
  );
}
