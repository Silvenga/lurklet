import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { MEDIA_PNG_BASE64 } from "@/_tests/mocks/fixtures";
import { Media } from "./Media";

test.each([
  "https://i.redd.it/example.png",
  "https://preview.redd.it/file%20name.png?width=640&x=2&x=1&s=a%2Bb+z&token=%252F",
  "https://example.com/not-allowed.png#fragment",
  "/relative/image.png",
])(
  "When an image source is %s then its complete value should only be sent through the media endpoint",
  (src) => {
    render(<Media src={src} alt="Example" title="Image title" />);

    const image = screen.getByRole("img", { name: "Example" });
    const url = new URL(image.getAttribute("src")!);
    expect(image.tagName).toBe("IMG");
    expect(url.origin).toBe(location.origin);
    expect(url.pathname).toBe("/api/media");
    expect(url.searchParams.get("url")).toBe(src);
    expect([...url.searchParams.keys()]).toEqual(["url"]);
    expect(url.hash).toBe("");
    expect(image).toHaveAttribute("title", "Image title");
    expect(image).toHaveAttribute("loading", "lazy");
    expect(image).toHaveAttribute("decoding", "async");
    expect(image).toHaveAttribute("referrerpolicy", "no-referrer");
    expect(image).toHaveClass("max-w-full", "h-auto", "self-start", "rounded-lg");
  },
);

test("When an image fails then it should stay unavailable without falling back to the upstream URL", () => {
  const src = "https://i.redd.it/missing.png";
  const { container, rerender } = render(<Media kind="image" src={src} alt="Missing image" />);

  fireEvent.error(screen.getByRole("img", { name: "Missing image" }));
  rerender(<Media kind="image" src={src} alt="Missing image" />);

  expect(screen.getByRole("img", { name: "Image unavailable: Missing image" }).tagName).toBe(
    "SPAN",
  );
  expect(container.querySelector("img, [src], [srcset], [href]")).toBeNull();
});

test("When the source changes after failure then the new image should load and returning to the original source should permit another attempt", () => {
  const first = "https://i.redd.it/first.png";
  const second = "https://i.redd.it/second.png";
  const { rerender } = render(<Media kind="image" src={first} alt="Example" />);
  const oldImage = screen.getByRole("img", { name: "Example" });
  fireEvent.error(oldImage);

  rerender(<Media kind="image" src={second} alt="Example" />);

  const nextImage = screen.getByRole("img", { name: "Example" });
  expect(new URL(nextImage.getAttribute("src")!).searchParams.get("url")).toBe(second);
  expect(nextImage).not.toBe(oldImage);

  fireEvent.error(oldImage);

  expect(screen.getByRole("img", { name: "Example" })).toBe(nextImage);

  rerender(<Media kind="image" src={first} alt="Example" />);

  expect(
    new URL(screen.getByRole("img", { name: "Example" }).getAttribute("src")!).searchParams.get(
      "url",
    ),
  ).toBe(first);
});

test.each([undefined, ""])(
  "When the image source is %j then no request URL should be rendered",
  (src) => {
    const { container } = render(<Media kind="image" src={src} />);

    expect(screen.getByRole("img", { name: "Image unavailable" })).toBeVisible();
    expect(container.querySelector("[src], [srcset]")).toBeNull();
  },
);

test("When media is a video then it should remain a non-fetching placeholder even if a source is supplied", () => {
  const { container } = render(<Media kind="video" src="https://v.redd.it/example.mp4" />);

  expect(screen.getByRole("img", { name: "Video unavailable" })).toBeVisible();
  expect(container.querySelector("img, video, source, [src], [srcset], [poster]")).toBeNull();
});

test("When extra image attributes include another URL then they should not bypass the proxy", () => {
  const props = {
    src: "https://i.redd.it/example.png",
    srcSet: "https://example.com/direct.png 2x",
    alt: "Example",
  };

  render(<Media {...props} />);

  expect(screen.getByRole("img", { name: "Example" })).not.toHaveAttribute("srcset");
});

test("When the media mock is requested then it should provide image bytes without a backend", async () => {
  const url = new URL("/api/media", location.origin);
  url.searchParams.set("url", "https://i.redd.it/example.png");

  const response = await fetch(url);

  expect(response.status).toBe(200);
  expect(response.headers.get("Content-Type")).toBe("image/png");
  expect(new Uint8Array(await response.arrayBuffer())).toEqual(
    Uint8Array.from(atob(MEDIA_PNG_BASE64), (char) => char.charCodeAt(0)),
  );
});
