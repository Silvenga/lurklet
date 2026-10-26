import type { ComponentProps } from "react";

export function Media({ alt, title }: Pick<ComponentProps<"img">, "src" | "alt" | "title">) {
  const label = alt ? `Media unavailable: ${alt}` : "Media unavailable";

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
