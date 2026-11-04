import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { Media } from "./Media";

test.each([
  "https://i.redd.it/example.png",
  "https://preview.redd.it/file%20name.png?width=640&x=2&x=1&s=a%2Bb+z&token=%252F",
  "https://preview.redd.it/nested/file%2F%25%3F%23.png?width=640&auto=webp&s=a%2Bb",
  "https://i.redd.it/file.png?",
  "https://example.com/not-allowed.png",
])(
  "When an image source is %s then its complete value should only be sent through the media endpoint",
  (src) => {
    render(<Media src={src} alt="Example" title="Image title" />);

    const image = screen.getByRole("img", { name: "Example" });
    const url = new URL(image.getAttribute("src")!);
    expect(image.tagName).toBe("IMG");
    expect(url.origin).toBe(location.origin);
    expect(url.href).toBe(`${location.origin}/api/media/${src.slice("https://".length)}`);
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
  expect(nextImage).toHaveAttribute("src", `${location.origin}/api/media/i.redd.it/second.png`);
  expect(nextImage).not.toBe(oldImage);

  fireEvent.error(oldImage);

  expect(screen.getByRole("img", { name: "Example" })).toBe(nextImage);

  rerender(<Media kind="image" src={first} alt="Example" />);

  expect(screen.getByRole("img", { name: "Example" })).toHaveAttribute(
    "src",
    `${location.origin}/api/media/i.redd.it/first.png`,
  );
});

test.each([
  undefined,
  "",
  "/relative/image.png",
  "not a URL",
  "https://i.redd.it/file.png#fragment",
  "http://i.redd.it/file.png",
  "https://user:pass@i.redd.it/file.png",
])("When the image source is %j then no request URL should be rendered", (src) => {
  const { container } = render(<Media kind="image" src={src} />);

  expect(screen.getByRole("img", { name: "Image unavailable" })).toBeVisible();
  expect(container.querySelector("[src], [srcset]")).toBeNull();
});

test("When a signature changes then the media path should remain stable and all query parameters should remain exposed", () => {
  const { rerender } = render(
    <Media src="https://preview.redd.it/file.png?width=640&auto=webp&s=first" alt="Example" />,
  );
  const first = new URL(screen.getByRole("img", { name: "Example" }).getAttribute("src")!);

  rerender(
    <Media src="https://preview.redd.it/file.png?width=640&auto=webp&s=second" alt="Example" />,
  );

  const second = new URL(screen.getByRole("img", { name: "Example" }).getAttribute("src")!);
  expect(first.pathname).toBe("/api/media/preview.redd.it/file.png");
  expect(second.pathname).toBe(first.pathname);
  expect([...first.searchParams]).toEqual([
    ["width", "640"],
    ["auto", "webp"],
    ["s", "first"],
  ]);
  expect([...second.searchParams]).toEqual([
    ["width", "640"],
    ["auto", "webp"],
    ["s", "second"],
  ]);
});

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

test.each([
  ["thumbnail", "size-16", "rounded-lg"],
  ["icon", "size-10", "rounded-full"],
] as const)(
  "When an image uses the %s variant then it should keep fixed dimensions and a compact failure placeholder",
  (variant, size, shape) => {
    const src = "https://styles.redditmedia.com/community.png?width=64&token=a%2Bb";
    const { container } = render(<Media kind="image" variant={variant} src={src} alt="Preview" />);
    const image = screen.getByRole("img", { name: "Preview" });
    const url = new URL(image.getAttribute("src")!);
    expect(url.origin).toBe(location.origin);
    expect(url.href).toBe(
      `${location.origin}/api/media/styles.redditmedia.com/community.png?width=64&token=a%2Bb`,
    );
    expect(image).toHaveClass(size, shape, "object-cover");
    expect(image).not.toHaveClass("h-auto");
    expect(image).toHaveAttribute("loading", "lazy");
    expect(image).toHaveAttribute("referrerpolicy", "no-referrer");

    fireEvent.error(image);

    const fallback = screen.getByRole("img", { name: "Image unavailable: Preview" });
    expect(fallback).toHaveClass(size, shape);
    expect(fallback).toHaveTextContent("");
    expect(fallback.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(container.querySelector("img, [src], [srcset]")).toBeNull();
  },
);
