import { useState, type ComponentProps } from "react";
import { client } from "@/clients/lurklet-api/generated/client.gen";
import type { GetMediaData } from "@/clients/lurklet-api/generated/types.gen";

export function Media({
  src,
  alt,
  title,
  kind,
}: Pick<ComponentProps<"img">, "src" | "alt" | "title"> & { kind?: "image" | "video" }) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  if (failedSource !== null && failedSource !== src) {
    setFailedSource(null);
  }

  if (src && kind !== "video" && failedSource !== src) {
    return (
      <img
        key={src}
        src={client.buildUrl({ url: "/api/media", query: { url: src } } satisfies GetMediaData)}
        alt={alt ?? (kind === "image" ? "Image" : "")}
        title={title}
        loading="lazy"
        decoding="async"
        referrerPolicy="no-referrer"
        className="inline-block h-auto max-w-full self-start rounded-lg align-middle"
        onError={() => setFailedSource(src)}
      />
    );
  }

  const type = kind === "image" ? "Image" : kind === "video" ? "Video" : "Media";
  const label = alt ? `${type} unavailable: ${alt}` : `${type} unavailable`;

  return (
    <span
      role="img"
      aria-label={label}
      title={title}
      className="text-muted-foreground wrap-anywhere"
    >
      [{label}]
    </span>
  );
}
