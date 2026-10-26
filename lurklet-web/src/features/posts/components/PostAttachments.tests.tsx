import { render, screen } from "@testing-library/react";
import { expect, onTestFinished, test, vi } from "vitest";
import type { PostAttachment } from "@/clients/lurklet-api/generated/types.gen";
import { PostAttachments } from "./PostAttachments";

test("When attachments mix links images and videos then their order and link policy should be preserved", () => {
  const attachments: PostAttachment[] = [
    { kind: "link", url: "https://example.com/first" },
    {
      kind: "image",
      url: "https://images.example.com/image.png",
      altText: "A diagram",
      caption: "Diagram caption",
    },
    {
      kind: "video",
      posterUrl: "https://images.example.com/poster.png",
      sources: [{ format: "mp4", url: "https://videos.example.com/video.mp4" }],
    },
    { kind: "link", url: "https://example.com/last" },
  ];

  const { container } = render(<PostAttachments attachments={attachments} />);

  expect([...container.children].map((element) => element.tagName)).toEqual([
    "A",
    "FIGURE",
    "SPAN",
    "A",
  ]);
  const image = screen.getByRole("img", { name: "A diagram" });
  expect(image.tagName).toBe("IMG");
  expect(new URL(image.getAttribute("src")!).searchParams.get("url")).toBe(
    "https://images.example.com/image.png",
  );
  expect(screen.getByText("Diagram caption").tagName).toBe("FIGCAPTION");
  expect(screen.getByRole("img", { name: "Video unavailable" })).toBeVisible();
  expect(screen.getAllByRole("link").map((link) => link.getAttribute("href"))).toEqual([
    "https://example.com/first",
    "https://example.com/last",
  ]);
  for (const link of screen.getAllByRole("link")) {
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow ugc external");
  }
});

test.each([null, ""])(
  "When an image has %j text fields then it should preserve its alt text choice without a caption",
  (text) => {
    const { container } = render(
      <PostAttachments
        attachments={[
          {
            kind: "image",
            url: "https://images.example.com/image.png",
            altText: text,
            caption: text,
          },
        ]}
      />,
    );

    expect(container.querySelector("img")).toHaveAttribute("alt", text ?? "Image");
    expect(container.querySelector("figcaption")).toBeNull();
  },
);

test("When image text contains markup then it should remain plain text", () => {
  const caption = '**Caption** <script>alert("example")</script>';

  const { container } = render(
    <PostAttachments
      attachments={[
        {
          kind: "image",
          url: "https://images.example.com/image.png",
          altText: "<em>Diagram</em>",
          caption,
        },
      ]}
    />,
  );

  expect(screen.getByRole("img", { name: "<em>Diagram</em>" })).toBeVisible();
  expect(screen.getByText(caption)).toBeVisible();
  expect(container.querySelector("script, em, strong")).toBeNull();
});

test.each([
  { sources: [] },
  {
    sources: [
      { format: "mp4", url: "https://videos.example.com/video.mp4" },
      { format: "hls", url: "https://videos.example.com/video.m3u8" },
      { format: "dash", url: "https://videos.example.com/video.mpd" },
    ],
  },
] satisfies Array<{ sources: Extract<PostAttachment, { kind: "video" }>["sources"] }>)(
  "When a video has sources $sources then it should render exactly one non-fetching placeholder",
  ({ sources }) => {
    const fetch = vi.spyOn(globalThis, "fetch");
    onTestFinished(() => fetch.mockRestore());

    const { container } = render(
      <PostAttachments
        attachments={[
          { kind: "video", posterUrl: "https://images.example.com/poster.png", sources },
        ]}
      />,
    );

    expect(screen.getAllByRole("img", { name: "Video unavailable" })).toHaveLength(1);
    expect(
      container.querySelector(
        "img, video, audio, iframe, source, object, embed, [src], [srcset], [poster], [href]",
      ),
    ).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  },
);

test("When no attachments are present then no placeholder should be rendered", () => {
  const { container } = render(<PostAttachments attachments={[]} />);

  expect(container).toBeEmptyDOMElement();
});
