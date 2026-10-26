import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { expect, test } from "vitest";
import { SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import type { Thumbnail } from "@/clients/lurklet-api/generated/types.gen";
import { PostList } from "./PostList";

const THUMBNAIL: Thumbnail = {
  url: "https://preview.redd.it/thumbnail.png?width=140&x=2&x=1&token=a%2Bb",
  width: 140,
  height: 100,
  obfuscatedUrl: "https://preview.redd.it/thumbnail.png?width=140&blur=40",
  isObfuscated: false,
};

test("When posts are returned then the list should preserve their order and show available metadata", () => {
  render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={SUBREDDIT_PAGE_RESPONSE.posts} />
    </MemoryRouter>,
  );

  expect(screen.getAllByRole("link").map((link) => link.textContent)).toEqual(
    SUBREDDIT_PAGE_RESPONSE.posts.map((post) => post.title),
  );
  expect(screen.getAllByText("Pinned")).toHaveLength(2);
  const first = screen.getAllByRole("listitem")[0]!;
  expect(first).toHaveClass("select-text");
  expect(within(first).getByText("u/seino_chan")).toBeVisible();
  expect(within(first).getByText("44 points")).toBeVisible();
  expect(within(first).getByText("2 comments")).toBeVisible();
  expect(first.querySelector("time")).toHaveAttribute("datetime", "2026-10-22T01:16:16.094Z");
});

test("When optional metadata is missing then the list should omit it without hiding zero counts", () => {
  const post = {
    ...SUBREDDIT_PAGE_RESPONSE.posts[0]!,
    title: null,
    authorName: null,
    createdAt: null,
    score: 0,
    commentCount: 0,
    isPinned: false,
  };

  const { container } = render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[post]} />
    </MemoryRouter>,
  );

  expect(screen.getByRole("link", { name: `Post ${post.id}` })).toHaveAttribute(
    "href",
    `/r/rust/comments/${post.id}`,
  );
  expect(screen.getByText("0 points")).toBeVisible();
  expect(screen.getByText("0 comments")).toBeVisible();
  expect(screen.queryByText(/^u\//)).not.toBeInTheDocument();
  expect(screen.queryByText("Pinned")).not.toBeInTheDocument();
  expect(container.querySelector("time")).toBeNull();
});

test("When counts are unknown then the list should not replace them with zero", () => {
  const post = { ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, score: null, commentCount: null };

  render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[post]} />
    </MemoryRouter>,
  );

  expect(screen.queryByText(/\bpoints?$/)).not.toBeInTheDocument();
  expect(screen.queryByText(/\bcomments?$/)).not.toBeInTheDocument();
});

test("When the listing is empty then it should show an empty state", () => {
  render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[]} />
    </MemoryRouter>,
  );

  expect(screen.getByText("No posts")).toBeVisible();
  expect(screen.queryByRole("list")).not.toBeInTheDocument();
});

test("When a post has a thumbnail then it should use the original proxied image without a reveal control", () => {
  const post = { ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, thumbnail: THUMBNAIL };
  render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[post]} />
    </MemoryRouter>,
  );

  const image = screen.getByRole("img", { name: `Thumbnail for ${post.title}` });
  const url = new URL(image.getAttribute("src")!);
  expect(url.origin).toBe(location.origin);
  expect(url.pathname).toBe("/api/media");
  expect(url.searchParams.get("url")).toBe(THUMBNAIL.url);
  expect(image).toHaveClass("size-16", "object-cover");
  expect(image.parentElement).not.toHaveClass("[&_img]:blur-md");
  expect(screen.queryByRole("button", { name: /Reveal thumbnail/ })).toBeNull();
  expect(screen.getByRole("link", { name: post.title })).toHaveAttribute(
    "href",
    `/r/rust/comments/${post.id}`,
  );
});

test.each(["NSFW", "spoiler", "obfuscated"])(
  "When a %s thumbnail is clicked then it should reveal the original through the proxy without navigating",
  async (reason) => {
    const user = userEvent.setup();
    const post = {
      ...SUBREDDIT_PAGE_RESPONSE.posts[0]!,
      isNsfw: reason === "NSFW",
      isSpoiler: reason === "spoiler",
      thumbnail: { ...THUMBNAIL, isObfuscated: reason === "obfuscated" },
    };
    const { container } = render(
      <MemoryRouter initialEntries={["/r/rust"]}>
        <Routes>
          <Route path="/r/rust" element={<PostList subreddit="rust" posts={[post]} />} />
          <Route path="*" element={<p>Left listing</p>} />
        </Routes>
      </MemoryRouter>,
    );
    const hiddenImage = container.querySelector("img")!;
    expect(new URL(hiddenImage.getAttribute("src")!).searchParams.get("url")).toBe(
      THUMBNAIL.obfuscatedUrl,
    );
    expect(hiddenImage.parentElement).toHaveClass("[&_img]:blur-md");
    expect(screen.queryByRole("img")).toBeNull();

    await user.click(screen.getByRole("button", { name: `Reveal thumbnail for ${post.title}` }));

    const image = screen.getByRole("img", { name: `Thumbnail for ${post.title}` });
    const url = new URL(image.getAttribute("src")!);
    expect(url.origin).toBe(location.origin);
    expect(url.pathname).toBe("/api/media");
    expect(url.searchParams.get("url")).toBe(THUMBNAIL.url);
    expect(image.parentElement).not.toHaveClass("[&_img]:blur-md");
    expect(screen.queryByRole("button", { name: /Reveal thumbnail/ })).toBeNull();
    expect(screen.queryByText("Left listing")).toBeNull();
    expect(screen.getByRole("link", { name: post.title })).toBeVisible();
  },
);

test.each(["{Enter}", " "])(
  "When a thumbnail has no obfuscated source then keyboard activation with %j should remove its CSS blur",
  async (key) => {
    const user = userEvent.setup();
    const post = {
      ...SUBREDDIT_PAGE_RESPONSE.posts[0]!,
      isSpoiler: true,
      thumbnail: { ...THUMBNAIL, obfuscatedUrl: null },
    };
    const { container } = render(
      <MemoryRouter>
        <PostList subreddit="rust" posts={[post]} />
      </MemoryRouter>,
    );
    const image = container.querySelector("img")!;
    expect(new URL(image.getAttribute("src")!).searchParams.get("url")).toBe(THUMBNAIL.url);
    expect(image.parentElement).toHaveClass("[&_img]:blur-md");

    await user.tab();
    expect(screen.getByRole("button", { name: /Reveal thumbnail/ })).toHaveFocus();
    await user.keyboard(key);

    expect(screen.getByRole("img", { name: `Thumbnail for ${post.title}` })).toBe(image);
    expect(image.parentElement).not.toHaveClass("[&_img]:blur-md");
  },
);

test("When a post has no thumbnail then it should render without an image or reveal control", () => {
  const post = { ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, thumbnail: null, isNsfw: true };
  const { container } = render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[post]} />
    </MemoryRouter>,
  );

  expect(container.querySelector("img, [src], [srcset]")).toBeNull();
  expect(screen.queryByRole("button", { name: /Reveal thumbnail/ })).toBeNull();
  expect(screen.getByRole("link", { name: post.title })).toBeVisible();
});

test("When a concealed thumbnail fails then it should not request the original until explicitly revealed", async () => {
  const user = userEvent.setup();
  const post = { ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, thumbnail: THUMBNAIL, isNsfw: true };
  const { container } = render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[post]} />
    </MemoryRouter>,
  );

  fireEvent.error(container.querySelector("img")!);

  expect(container.querySelector("img, [src], [srcset]")).toBeNull();
  expect(screen.getByRole("link", { name: post.title })).toBeVisible();

  await user.click(screen.getByRole("button", { name: /Reveal thumbnail/ }));

  const image = screen.getByRole("img", { name: `Thumbnail for ${post.title}` });
  expect(new URL(image.getAttribute("src")!).searchParams.get("url")).toBe(THUMBNAIL.url);

  fireEvent.error(image);

  expect(
    screen.getByRole("img", { name: `Image unavailable: Thumbnail for ${post.title}` }),
  ).toBeVisible();
  expect(container.querySelector("img, [src], [srcset]")).toBeNull();
});

test("When one thumbnail is revealed then other posts should remain concealed and a changed source should reset the reveal", async () => {
  const user = userEvent.setup();
  const first = { ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, thumbnail: THUMBNAIL, isSpoiler: true };
  const second = { ...first, id: "second", title: "Second post" };
  const { rerender } = render(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[first, second]} />
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: `Reveal thumbnail for ${first.title}` }));
  rerender(
    <MemoryRouter>
      <PostList subreddit="rust" posts={[{ ...first }, second]} />
    </MemoryRouter>,
  );

  expect(screen.getByRole("img", { name: `Thumbnail for ${first.title}` })).toBeVisible();
  expect(screen.getByRole("button", { name: "Reveal thumbnail for Second post" })).toBeVisible();

  rerender(
    <MemoryRouter>
      <PostList
        subreddit="rust"
        posts={[
          { ...first, thumbnail: { ...THUMBNAIL, url: "https://i.redd.it/replacement.png" } },
        ]}
      />
    </MemoryRouter>,
  );

  expect(screen.getByRole("button", { name: `Reveal thumbnail for ${first.title}` })).toBeVisible();
  expect(screen.queryByRole("img")).toBeNull();
});
