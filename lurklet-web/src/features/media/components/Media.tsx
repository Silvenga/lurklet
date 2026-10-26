import type { ComponentProps } from "react";

export function Media({
  alt,
  title,
  kind,
}: Pick<ComponentProps<"img">, "src" | "alt" | "title"> & { kind?: "image" | "video" }) {
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
