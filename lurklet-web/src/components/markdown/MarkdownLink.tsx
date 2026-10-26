import type { ComponentProps } from "react";
import { Link, useLocation } from "react-router";
import { ExternalLink } from "@/components/ExternalLink";

export function MarkdownLink({
  href,
  title,
  children,
}: Pick<ComponentProps<"a">, "href" | "title" | "children">) {
  const location = useLocation();

  if (!href) {
    return <span title={title}>{children}</span>;
  }
  if (href.startsWith("#")) {
    return (
      <a href={href} title={title}>
        {children}
      </a>
    );
  }

  const origin = globalThis.location.origin;
  let url: URL;
  try {
    url = new URL(href, `${origin}${location.pathname}${location.search}`);
  } catch {
    return <span title={title}>{children}</span>;
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return (
      <a href={href} title={title}>
        {children}
      </a>
    );
  }

  const hostname = url.hostname.replace(/\.$/, "");
  const isReddit =
    hostname === "reddit.com" ||
    hostname.endsWith(".reddit.com") ||
    hostname === "redd.it" ||
    hostname === "www.redd.it";
  if (isReddit || url.origin === origin) {
    const path = `${url.pathname}${url.search}${url.hash}`;
    // Keep double-slash paths local without the router normalizing away their extra slashes.
    if (path.startsWith("//")) {
      return (
        <a href={`/.${path}`} title={title}>
          {children}
        </a>
      );
    }
    return (
      <Link to={path} title={title}>
        {children}
      </Link>
    );
  }

  return (
    <ExternalLink href={href} title={title}>
      {children}
    </ExternalLink>
  );
}
