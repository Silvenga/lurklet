import { useState } from "react";
import type { VideoSource } from "@/clients/lurklet-api/generated/types.gen";
import { buildMediaUrl } from "../build-media-url";
import { Media } from "./Media";

export function Video({
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
  const [failure, setFailure] = useState<{ sourceKey: string; urls: string[] }>({
    sourceKey,
    urls: [],
  });
  if (failure.sourceKey !== sourceKey) {
    setFailure({ sourceKey, urls: [] });
  }
  if (
    urls.length === 0 ||
    (failure.sourceKey === sourceKey && failure.urls.length === urls.length)
  ) {
    return <Media kind="video" />;
  }

  return (
    // oxlint-disable-next-line jsx-a11y/media-has-caption - Reddit's MP4 attachment contract does not provide captions.
    <video
      key={sourceKey}
      aria-label="Video"
      poster={buildMediaUrl(posterUrl)}
      controls
      playsInline
      preload="none"
      className="h-auto max-h-dvh w-full self-start rounded-lg"
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
  );
}
