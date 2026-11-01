import type { ComponentProps } from "react";
import { Link, useLocation } from "react-router";
import { ExternalLink } from "@/components/ExternalLink";

export function ContentLink({
  href,
  title,
  children,
  className,
}: Pick<ComponentProps<"a">, "href" | "title" | "children" | "className">) {
  const location = useLocation();

  if (!href) {
    return (
      <span title={title} className={className}>
        {children}
      </span>
    );
  }
  if (href.startsWith("#")) {
    return (
      <a href={href} title={title} className={className}>
        {children}
      </a>
    );
  }

  const origin = globalThis.location.origin;
  let url: URL;
  try {
    url = new URL(href, `${origin}${location.pathname}${location.search}`);
  } catch {
    return (
      <span title={title} className={className}>
        {children}
      </span>
    );
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return (
      <a href={href} title={title} className={className}>
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
        <a href={`/.${path}`} title={title} className={className}>
          {children}
        </a>
      );
    }
    return (
      <Link to={path} title={title} className={className}>
        {children}
      </Link>
    );
  }

  return (
    <ExternalLink href={href} title={title} className={className}>
      {children}
    </ExternalLink>
  );
}
