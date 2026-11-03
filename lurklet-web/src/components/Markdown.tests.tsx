import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { expect, test } from "vitest";
import type { MarkdownMedia } from "@/clients/lurklet-api/generated/types.gen";
import { Markdown } from "./Markdown";

function renderMarkdown(content: string, path = "/", media: MarkdownMedia[] = []) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Markdown value={{ content, media }} />
    </MemoryRouter>,
  );
}

test("When Markdown uses GFM extensions then it should render tables tasks strikethrough and autolinks", () => {
  const content = [
    "| Name | Value |",
    "| --- | --- |",
    "| Example | 42 |",
    "",
    "- [x] Complete",
    "- [ ] Pending",
    "",
    "~~Removed~~",
    "",
    "https://example.com",
  ].join("\n");

  renderMarkdown(content);

  const table = screen.getByRole("table");
  expect(within(table).getByRole("columnheader", { name: "Name" })).toBeInTheDocument();
  expect(within(table).getByRole("cell", { name: "42" })).toBeInTheDocument();
  const [complete, pending] = screen.getAllByRole("checkbox");
  expect(complete).toBeChecked();
  expect(complete).toBeDisabled();
  expect(pending).not.toBeChecked();
  expect(pending).toBeDisabled();
  expect(screen.getByText("Removed").tagName).toBe("DEL");
  expect(screen.getByRole("link", { name: "https://example.com" })).toHaveAttribute(
    "href",
    "https://example.com",
  );
});

test("When Markdown contains links and images then links should use their policy and images should use the media proxy", () => {
  const content = [
    "[Subreddit](/r/example)",
    "[External](https://example.com/post)",
    '![Example image](https://images.example.test/example.png "Image title")',
  ].join("\n\n");

  const { container } = renderMarkdown(content);

  expect(screen.getByRole("link", { name: "Subreddit" })).toHaveAttribute("href", "/r/example");
  expect(screen.getByRole("link", { name: "External" })).toHaveAttribute(
    "href",
    "https://example.com/post",
  );
  expect(screen.getByRole("link", { name: "External" })).toHaveAttribute("target", "_blank");
  expect(screen.getByRole("link", { name: "External" })).toHaveAttribute(
    "rel",
    "noopener noreferrer nofollow ugc external",
  );
  const icon = screen.getByRole("link", { name: "External" }).querySelector("svg");
  expect(icon).toHaveAttribute("aria-hidden", "true");
  expect(icon).toHaveAttribute("focusable", "false");
  const image = screen.getByRole("img", { name: "Example image" });
  const url = new URL(image.getAttribute("src")!);
  expect(url.origin).toBe(location.origin);
  expect(url.pathname).toBe("/api/media");
  expect(url.searchParams.get("url")).toBe("https://images.example.test/example.png");
  expect(image).toHaveAttribute("title", "Image title");
  expect(container.querySelector("video, audio, iframe, source")).toBeNull();
});

test("When Markdown contains raw HTML then it should render as text rather than executable elements", () => {
  const content = '<script>alert("example")</script>\n\n<strong>Not markup</strong>';

  const { container } = renderMarkdown(content);

  expect(container.querySelector("script, strong")).toBeNull();
  expect(container).toHaveTextContent('<script>alert("example")</script>');
  expect(container).toHaveTextContent("<strong>Not markup</strong>");
});

test.each([
  "https://reddit.com",
  "https://www.reddit.com",
  "https://old.reddit.com",
  "https://new.reddit.com",
  "https://np.reddit.com",
  "https://m.reddit.com",
  "https://sh.reddit.com",
  "https://en.reddit.com",
  "http://www.reddit.com",
  "//www.reddit.com",
  "https://WWW.REDDIT.COM",
  "https://www.reddit.com.",
])(
  "When a Markdown link uses %s then it should become a local link with its query and fragment intact",
  (origin) => {
    renderMarkdown(`[Discussion](${origin}/r/rust/comments/1xc54av/title?sort=new#comment)`);

    const link = screen.getByRole("link", { name: "Discussion" });
    expect(link).toHaveAttribute("href", "/r/rust/comments/1xc54av/title?sort=new#comment");
    expect(link).not.toHaveAttribute("target");
    expect(link).not.toHaveAttribute("rel");
    expect(link).not.toHaveAttribute("node");
  },
);

test.each([
  ["https://www.reddit.com/r/rust/wiki/rules", "/r/rust/wiki/rules"],
  ["https://www.reddit.com/r/rust/search?q=async%20await", "/r/rust/search?q=async%20await"],
  ["https://www.reddit.com/user/example", "/user/example"],
  ["https://www.reddit.com/r/rust/s/share-token", "/r/rust/s/share-token"],
  [
    "https://www.reddit.com/r/rust/comments/post/title/comment",
    "/r/rust/comments/post/title/comment",
  ],
  ["https://redd.it/1xc54av", "/1xc54av"],
  ["https://www.redd.it/1xc54av", "/1xc54av"],
])("When %s is a Reddit destination then it should be rewritten to %s", (url, path) => {
  renderMarkdown(`[Reddit](${url})`);

  expect(screen.getByRole("link", { name: "Reddit" })).toHaveAttribute("href", path);
});

test("When Reddit plugins are composed then headings spoilers superscript mentions and strikethrough should render together", async () => {
  const user = userEvent.setup();
  const content = "##Heading\n\n>!^(**Secret**) r/rust u/example ~~removed~~ ~literal~ ^word!<";
  const { container } = renderMarkdown(content);
  expect(screen.getByRole("heading", { level: 2, name: "Heading" })).toBeVisible();
  expect(screen.getByText("Secret").closest("[aria-hidden=true]")).toHaveAttribute("inert");
  expect(screen.queryByRole("link")).toBeNull();

  await user.click(screen.getByRole("button", { name: "Show spoiler" }));

  expect(screen.getByText("Secret").closest("sup")).toBeVisible();
  expect(screen.getByText("word").tagName).toBe("SUP");
  expect(screen.getByRole("link", { name: "r/rust" })).toHaveAttribute("href", "/r/rust");
  expect(screen.getByRole("link", { name: "u/example" })).toHaveAttribute("href", "/u/example");
  expect(screen.getByText("removed").tagName).toBe("DEL");
  expect(container).toHaveTextContent("~literal~");
  expect(container.querySelector("blockquote, button a, button button")).toBeNull();
});

test("When a link label contains a spoiler then the reveal control should conceal the link and remain outside it", async () => {
  const user = userEvent.setup();
  const { container } = renderMarkdown("[Before >!secret!< after](https://example.com)");
  expect(screen.queryByRole("link")).toBeNull();

  await user.click(screen.getByRole("button", { name: "Show spoiler" }));

  expect(screen.getByRole("link", { name: "Before secret after" })).toHaveAttribute(
    "href",
    "https://example.com",
  );
  expect(container.querySelector("a button, button a")).toBeNull();
});

test.each([
  "https://example.com/article?from=reddit#section",
  "//example.com/article",
  "https://reddit.com.example.org/r/rust",
  "https://notreddit.com/r/rust",
  "https://reddit.com@example.org/r/rust",
  "https://i.redd.it/image.png",
  "https://preview.redd.it/image.png?width=640",
  "https://v.redd.it/video",
  "https://styles.redditmedia.com/icon.png",
])(
  "When %s is external or a media destination then it should retain its URL and use ExternalLink",
  (url) => {
    renderMarkdown(`[External](${url} "Destination title")`);

    const link = screen.getByRole("link", { name: "External" });
    expect(link).toHaveAttribute("href", url);
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow ugc external");
    expect(link).toHaveAttribute("title", "Destination title");
    expect(screen.queryByRole("img")).toBeNull();
  },
);

test.each([
  ["/r/rust?sort=new#posts", "/r/rust?sort=new#posts"],
  ["../other", "/r/rust/comments/other"],
  ["./other", "/r/rust/comments/1xc54av/other"],
  ["?sort=new#comments", "/r/rust/comments/1xc54av/title?sort=new#comments"],
])(
  "When a Markdown link is %s then it should resolve against the current URL as %s",
  (href, expected) => {
    renderMarkdown(`[Local](${href})`, "/r/rust/comments/1xc54av/title");

    expect(screen.getByRole("link", { name: "Local" })).toHaveAttribute("href", expected);
    expect(screen.getByRole("link", { name: "Local" })).not.toHaveAttribute("target");
  },
);

test("When a Markdown link uses the application origin then it should use local navigation", () => {
  renderMarkdown(`[Local](${globalThis.location.origin}/r/rust?sort=top&t=week)`);

  expect(screen.getByRole("link", { name: "Local" })).toHaveAttribute(
    "href",
    "/r/rust?sort=top&t=week",
  );
  expect(screen.getByRole("link", { name: "Local" })).not.toHaveAttribute("target");
});

test.each(["#section", "mailto:someone@example.com", "irc://example.com/channel"])(
  "When a Markdown link is %s then its native anchor behavior should be preserved",
  (href) => {
    renderMarkdown(`[Destination](${href})`);

    expect(screen.getByRole("link", { name: "Destination" })).toHaveAttribute("href", href);
    expect(screen.getByRole("link", { name: "Destination" })).not.toHaveAttribute("target");
    expect(screen.getByRole("link", { name: "Destination" })).not.toHaveAttribute("data-discover");
  },
);

test.each([
  "javascript:alert%281%29",
  "JaVaScRiPt:alert%281%29",
  "data:text/html,example",
  "vbscript:example",
  "https://%",
])(
  "When a Markdown destination is unsafe or invalid (%s) then it should not become a clickable link",
  (href) => {
    renderMarkdown(`[Blocked](${href})`);

    expect(screen.getByText("Blocked")).toBeVisible();
    expect(screen.queryByRole("link")).toBeNull();
  },
);

test("When a Reddit path begins with two slashes then rewriting should not turn it into an external host", () => {
  renderMarkdown("[Local](https://www.reddit.com//example.org/path?query=value#fragment)");

  const link = screen.getByRole("link", { name: "Local" });
  const url = new URL(link.getAttribute("href")!, globalThis.location.origin);
  expect(url.origin).toBe(globalThis.location.origin);
  expect(url.pathname).toBe("//example.org/path");
  expect(url.search).toBe("?query=value");
  expect(url.hash).toBe("#fragment");
});

test("When an image is inline inside a link then its proxied image and failure placeholder should preserve valid paragraph structure", () => {
  const { container } = renderMarkdown(
    'Before [![A Rust diagram](https://i.redd.it/example.png "Diagram title")](https://www.reddit.com/r/rust) after.',
  );

  const media = screen.getByRole("img", { name: "A Rust diagram" });
  const link = screen.getByRole("link", { name: "A Rust diagram" });
  expect(media.tagName).toBe("IMG");
  expect(media).toHaveAttribute("title", "Diagram title");
  expect(link).toHaveAttribute("href", "/r/rust");
  expect(link).toContainElement(media);
  expect(media.closest("p")).toHaveTextContent("Before after.");
  expect(container.querySelector("p div, p figure, video, audio, iframe, source")).toBeNull();

  fireEvent.error(media);

  const placeholder = screen.getByRole("img", { name: "Media unavailable: A Rust diagram" });
  expect(link).toContainElement(placeholder);
  expect(placeholder.closest("p")).toHaveTextContent(
    "Before [Media unavailable: A Rust diagram] after.",
  );
  expect(container.querySelector("p div, p figure, img")).toBeNull();
});

test.each(["https://i.redd.it/example.png", "/media/example.png"])(
  "When inline media uses %s without alt text then its source should only be passed to the media API",
  (src) => {
    const { container } = renderMarkdown(`![](${src})`);

    const image = container.querySelector("img");
    expect(image).toHaveAttribute("alt", "");
    const url = new URL(image!.getAttribute("src")!);
    expect(url.origin).toBe(location.origin);
    expect(url.pathname).toBe("/api/media");
    expect(url.searchParams.get("url")).toBe(src);
    expect(
      container.querySelector("[srcset], video, audio, iframe, source, object, embed"),
    ).toBeNull();
  },
);

test("When the Markdown URL filter removes an unsafe image source then it should remain a non-fetching placeholder", () => {
  const { container } = renderMarkdown("![](data:image/png;base64,example)");

  expect(screen.getByRole("img", { name: "Media unavailable" })).toBeVisible();
  expect(container.querySelector("img, [src], [srcset]")).toBeNull();
});

test("When raw HTML contains media then it should remain text rather than create media or embeds", () => {
  const html =
    '<img src="https://example.com/image.png">\n\n<video src="https://example.com/video.mp4"></video>\n\n<iframe src="https://example.com/embed"></iframe>';

  const { container } = renderMarkdown(html);

  expect(container.querySelector("img, video, iframe")).toBeNull();
  expect(container).toHaveTextContent('<img src="https://example.com/image.png">');
  expect(screen.queryByRole("img")).toBeNull();
});

test("When a media URL is only linked or autolinked then it should not become embedded media", () => {
  const { container } = renderMarkdown(
    "[Video](https://v.redd.it/example)\n\nhttps://i.redd.it/example.png",
  );

  expect(screen.getAllByRole("link")).toHaveLength(2);
  expect(screen.getByRole("link", { name: "Video" })).toHaveAttribute(
    "href",
    "https://v.redd.it/example",
  );
  expect(screen.queryByRole("img")).toBeNull();
  expect(container.querySelector("img, video, iframe")).toBeNull();
});

test.each([
  ["asset", 'Before ![An image](asset "Caption") after.'],
  ["giphy|5L2okTiupVRxC", 'Before ![An image](giphy|5L2okTiupVRxC "Caption") after.'],
  ["asset", 'Before ![An image][media] after.\n\n[media]: asset "Caption"'],
])(
  "When Markdown references %s then its image should resolve before URL encoding and retain layout metadata",
  (id, content) => {
    const media = [
      {
        id,
        mimeType: "image/gif",
        url: "https://external-preview.redd.it/asset.gif?width=300&auto=webp&s=a%2Bb",
        width: 300,
        height: 200,
      },
    ];

    const { container } = renderMarkdown(content, "/", media);

    const image = screen.getByRole("img", { name: "An image" });
    const url = new URL(image.getAttribute("src")!);
    expect(url.origin).toBe(location.origin);
    expect(url.pathname).toBe("/api/media");
    expect(url.searchParams.get("url")).toBe(media[0]!.url);
    expect(image).toHaveAttribute("width", "300");
    expect(image).toHaveAttribute("height", "200");
    expect(image).toHaveAttribute("title", "Caption");
    expect(image.closest("p")).toHaveTextContent("Before after.");
    expect(container.querySelector("p div, p figure, video, iframe, [srcset]")).toBeNull();
  },
);

test.each([
  { media: [] },
  {
    media: [
      {
        id: "asset",
        mimeType: "video/mp4",
        url: "https://v.redd.it/file.mp4",
        width: null,
        height: null,
      },
    ],
  },
  {
    media: [
      { id: "asset", mimeType: "image/png", url: "javascript:alert(1)", width: null, height: null },
    ],
  },
  {
    media: [
      {
        id: "asset",
        mimeType: null,
        url: "data:image/png;base64,example",
        width: null,
        height: null,
      },
    ],
  },
])(
  "When an inline reference is unresolved unsupported or unsafe then it should remain a non-fetching placeholder (%j)",
  ({ media }) => {
    const { container } = renderMarkdown("Before ![An image](asset) after.", "/", media);

    expect(screen.getByRole("img", { name: "Media unavailable: An image" })).toBeVisible();
    expect(container).toHaveTextContent("Before [Media unavailable: An image] after.");
    expect(container.querySelector("[src], [srcset], video, iframe")).toBeNull();
  },
);
