import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { expect, test } from "vitest";
import { SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import { PostList } from "./PostList";

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
