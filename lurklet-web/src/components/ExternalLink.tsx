import { ExternalLinkIcon } from "lucide-react";
import type { ComponentProps } from "react";

export function ExternalLink({ children, ...props }: Omit<ComponentProps<"a">, "target" | "rel">) {
  return (
    <a {...props} target="_blank" rel="noopener noreferrer nofollow ugc external">
      {children}
      <ExternalLinkIcon
        aria-hidden="true"
        focusable="false"
        className="ml-1 inline size-[1em] align-middle"
      />
    </a>
  );
}
