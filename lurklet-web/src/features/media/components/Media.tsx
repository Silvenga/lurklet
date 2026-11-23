import { cn } from "cn";
import { ImageOffIcon } from "lucide-react";
import { useState, type ComponentProps } from "react";
import { Button } from "@/components/ui/button";
import { buildMediaUrl } from "../build-media-url";

export function Media({
  src,
  alt,
  title,
  width,
  height,
  kind,
  variant = "content",
}: Pick<ComponentProps<"img">, "src" | "alt" | "title" | "width" | "height"> & {
  kind?: "image" | "video";
  variant?: "content" | "thumbnail" | "icon" | "emoji";
}) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const [canRetry, setCanRetry] = useState(false);
  if (failedSource !== null && failedSource !== src) {
    setFailedSource(null);
  }
  const compactClassName = cn(
    variant !== "content" && "shrink-0 select-none",
    variant === "thumbnail" && "size-16 rounded-lg",
    variant === "icon" && "size-10 rounded-full",
  );

  const proxyUrl = buildMediaUrl(src);
  if (src && proxyUrl && kind !== "video" && failedSource !== src) {
    return (
      <img
        key={src}
        src={proxyUrl}
        alt={alt ?? (kind === "image" ? "Image" : "")}
        title={title}
        width={width}
        height={height}
        loading="lazy"
        decoding="async"
        referrerPolicy="no-referrer"
        className={cn(
          variant === "content"
            ? "inline-block h-auto max-w-full self-start rounded-lg align-middle"
            : variant === "emoji"
              ? "inline-block size-4 object-contain align-text-bottom"
              : "bg-muted block object-cover",
          compactClassName,
        )}
        onError={(event) => {
          setCanRetry(!event.currentTarget.closest("a, button"));
          setFailedSource(src);
        }}
      />
    );
  }

  if (variant === "emoji") {
    return <span>{alt}</span>;
  }

  const type = kind === "image" ? "Image" : kind === "video" ? "Video" : "Media";
  const label = alt ? `${type} unavailable: ${alt}` : `${type} unavailable`;

  if (variant === "content") {
    const message = failedSource
      ? "Image failed to load"
      : kind === "video"
        ? "No playable video source"
        : "No usable image source";
    return (
      <span className="text-muted-foreground text-sm wrap-anywhere">
        {message}
        {alt ? `: ${alt}` : "."}
        {failedSource && canRetry && (
          <>
            {" "}
            <Button size="xs" variant="outline" onPress={() => setFailedSource(null)}>
              Retry
            </Button>
          </>
        )}
      </span>
    );
  }

  return (
    <span
      role="img"
      aria-label={label}
      title={title}
      className={cn(
        "text-muted-foreground",
        "bg-muted inline-flex items-center justify-center",
        compactClassName,
      )}
    >
      <ImageOffIcon aria-hidden="true" className="size-4" />
    </span>
  );
}
