import { cn } from "cn";
import { useRef, useState } from "react";
import type { VideoSource } from "@/clients/lurklet-api/generated/types.gen";
import { LoadFailureNotice } from "@/components/LoadFailureNotice";
import { Alert, AlertTitle } from "@/components/ui/alert";
import { buildMediaUrl } from "../build-media-url";

export function Mp4Video({
  sources,
  posterUrl,
}: {
  sources: VideoSource[];
  posterUrl: string | null;
}) {
  const urls = [
    ...new Set(
      sources
        .filter((source) => source.format === "mp4")
        .map((source) => buildMediaUrl(source.url))
        .filter((url) => url !== undefined),
    ),
  ];
  const sourceKey = JSON.stringify(urls);
  const player = useRef<HTMLVideoElement>(null);
  const [failure, setFailure] = useState<{ sourceKey: string; urls: string[] }>({
    sourceKey,
    urls: [],
  });
  if (failure.sourceKey !== sourceKey) {
    setFailure({ sourceKey, urls: [] });
  }
  if (urls.length === 0) {
    return (
      <Alert>
        <AlertTitle>No playable video source</AlertTitle>
      </Alert>
    );
  }
  const failed = failure.sourceKey === sourceKey && failure.urls.length === urls.length;

  return (
    <div className="bg-muted relative w-full rounded-lg">
      {/* oxlint-disable-next-line jsx-a11y/media-has-caption - Reddit's MP4 attachment contract does not provide captions. */}
      <video
        ref={player}
        key={sourceKey}
        aria-label="Video"
        poster={buildMediaUrl(posterUrl)}
        controls
        playsInline
        preload="none"
        className={cn("block h-auto max-h-[90dvh] w-full rounded-lg", failed && "invisible")}
        onError={(event) => {
          if (event.target === event.currentTarget) {
            setFailure((current) =>
              current.sourceKey === sourceKey ? { sourceKey, urls } : current,
            );
          }
        }}
      >
        {urls.map((url) => (
          <source
            key={url}
            src={url}
            type="video/mp4"
            onError={() =>
              setFailure((current) =>
                current.sourceKey === sourceKey && !current.urls.includes(url)
                  ? { sourceKey, urls: [...current.urls, url] }
                  : current,
              )
            }
          />
        ))}
      </video>
      {failed && (
        <div className="absolute inset-0 flex items-center justify-center p-3">
          <div className="w-full max-w-xs">
            <LoadFailureNotice
              subject="Video"
              onRetry={() => {
                setFailure({ sourceKey, urls: [] });
                player.current?.load();
                void player.current?.play().catch(() => {});
              }}
            />
          </div>
        </div>
      )}
    </div>
  );
}
