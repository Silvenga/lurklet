import type { ComponentProps } from "react";
import { ContentLink } from "@/components/ContentLink";

export function MarkdownLink({
  href,
  title,
  children,
}: Pick<ComponentProps<"a">, "href" | "title" | "children">) {
  return (
    <ContentLink href={href} title={title}>
      {children}
    </ContentLink>
  );
}
