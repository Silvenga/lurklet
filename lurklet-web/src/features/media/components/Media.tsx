import { cn } from "cn";
import { ImageOffIcon } from "lucide-react";
import { useState, type ComponentProps } from "react";
import { client } from "@/clients/lurklet-api/generated/client.gen";
import type { GetMediaData } from "@/clients/lurklet-api/generated/types.gen";

export function Media({
  src,
  alt,
  title,
  kind,
  variant = "content",
}: Pick<ComponentProps<"img">, "src" | "alt" | "title"> & {
  kind?: "image" | "video";
  variant?: "content" | "thumbnail" | "icon";
}) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  if (failedSource !== null && failedSource !== src) {
    setFailedSource(null);
  }
  const compactClassName = cn(
    variant !== "content" && "shrink-0 select-none",
    variant === "thumbnail" && "size-16 rounded-lg",
    variant === "icon" && "size-10 rounded-full",
  );

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
        className={cn(
          variant === "content"
            ? "inline-block h-auto max-w-full self-start rounded-lg align-middle"
            : "bg-muted block object-cover",
          compactClassName,
        )}
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
      className={cn(
        "text-muted-foreground",
        variant === "content"
          ? "wrap-anywhere"
          : "bg-muted inline-flex items-center justify-center",
        compactClassName,
      )}
    >
      {variant === "content" ? (
        `[${label}]`
      ) : (
        <ImageOffIcon aria-hidden="true" className="size-4" />
      )}
    </span>
  );
}
