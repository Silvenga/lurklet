import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import type { VideoSource } from "@/clients/lurklet-api/generated/types.gen";
import { Video } from "./Video";

test("When MP4 sources and a poster are available then playback should use only proxied URLs without autoplay or preloading", () => {
  const url = "https://packaged-media.redd.it/file.mp4?s=a%2Bb&tag=2&tag=1";
  const posterUrl = "https://preview.redd.it/poster.jpeg?s=opaque";
  const sources: VideoSource[] = [
    { format: "hls", url: "https://v.redd.it/file.m3u8" },
    { format: "mp4", url },
    { format: "dash", url: "https://v.redd.it/file.mpd" },
  ];

  render(<Video sources={sources} posterUrl={posterUrl} />);

  const video = screen.getByLabelText("Video");
  expect(video.tagName).toBe("VIDEO");
  for (const [value, original] of [
    [video.querySelector("source")!.getAttribute("src"), url],
    [video.getAttribute("poster"), posterUrl],
  ]) {
    const proxy = new URL(value!);
    expect(proxy.origin).toBe(location.origin);
    expect(proxy.href).toBe(`${location.origin}/api/media/${original!.slice("https://".length)}`);
  }
  expect(video).toHaveAttribute("controls");
  expect(video).toHaveAttribute("playsinline");
  expect(video).toHaveAttribute("preload", "none");
  expect(video).not.toHaveAttribute("autoplay");
  expect(video.querySelectorAll("source")).toHaveLength(1);
  expect(video.querySelector("source")).toHaveAttribute("type", "video/mp4");
});

test("When playback fails then distinct MP4 alternatives should be tried in order before becoming unavailable", () => {
  const first: VideoSource = { format: "mp4", url: "https://v.redd.it/first.mp4" };
  const second: VideoSource = { format: "mp4", url: "https://v.redd.it/second.mp4" };
  const { container, rerender } = render(
    <Video sources={[first, first, second]} posterUrl={null} />,
  );
  const original = screen.getByLabelText("Video");
  const candidates = original.querySelectorAll("source");
  expect([...candidates].map((source) => source.src)).toEqual([
    `${location.origin}/api/media/v.redd.it/first.mp4`,
    `${location.origin}/api/media/v.redd.it/second.mp4`,
  ]);

  fireEvent.error(candidates[0]!);

  const alternative = screen.getByLabelText("Video");
  expect(alternative).toBe(original);
  expect(alternative).not.toHaveAttribute("poster");
  fireEvent.error(candidates[0]!);
  expect(screen.getByLabelText("Video")).toBe(alternative);

  fireEvent.error(candidates[1]!);

  expect(screen.getByRole("img", { name: "Video unavailable" })).toBeVisible();
  expect(container.querySelector("video, [src], [poster], [href]")).toBeNull();

  rerender(<Video sources={[second]} posterUrl={null} />);

  expect(screen.getByLabelText("Video").querySelector("source")).toHaveAttribute(
    "src",
    `${location.origin}/api/media/v.redd.it/second.mp4`,
  );
  fireEvent.error(alternative);
  expect(screen.getByLabelText("Video")).toBeVisible();

  fireEvent.error(screen.getByLabelText("Video"));

  expect(screen.getByRole("img", { name: "Video unavailable" })).toBeVisible();
});

test("When video sources or posters are invalid then they should not create requests or prevent valid alternatives", () => {
  const { container, rerender } = render(
    <Video
      sources={[
        { format: "mp4", url: "/relative.mp4" },
        { format: "mp4", url: "not a URL" },
        { format: "mp4", url: "https://v.redd.it/nested/file.mp4?s=signature" },
      ]}
      posterUrl="/relative.png"
    />,
  );

  const video = screen.getByLabelText("Video");
  expect(video).not.toHaveAttribute("poster");
  expect(video.querySelectorAll("source")).toHaveLength(1);
  expect(video.querySelector("source")).toHaveAttribute(
    "src",
    `${location.origin}/api/media/v.redd.it/nested/file.mp4?s=signature`,
  );

  rerender(<Video sources={[{ format: "mp4", url: "/relative.mp4" }]} posterUrl={null} />);

  expect(screen.getByRole("img", { name: "Video unavailable" })).toBeVisible();
  expect(container.querySelector("video, [src], [poster]")).toBeNull();
});
