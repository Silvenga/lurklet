import { QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw/http";
import { createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test } from "vitest";
import { COMMENTS_RESPONSE, POST_RESPONSE, SUBREDDIT_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { createQueryClient } from "@/app/query-client";
import { APP_ROUTES } from "@/app/router";
import type { CommentItem } from "@/clients/lurklet-api/generated/types.gen";

const POST_PATH = `/r/rust/comments/${POST_RESPONSE.id}`;

function commentItem(
  id: string,
  bodyMarkdown: string,
  replies: CommentItem[] = [],
): Extract<CommentItem, { kind: "comment" }> {
  return {
    kind: "comment",
    comment: { ...COMMENTS_RESPONSE.items[0]!.comment, id, author: { id, name: id }, bodyMarkdown },
    replies,
  };
}

function renderPath(path = POST_PATH) {
  const queryClient = createQueryClient();
  const router = createMemoryRouter(APP_ROUTES, { initialEntries: [path] });
  const { unmount } = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  onTestFinished(() => {
    unmount();
    router.dispose();
    queryClient.clear();
  });
  return router;
}

test("When comments resolve first then they should render independently of the post and metadata", async () => {
  const comments = Promise.withResolvers<Response>();
  const post = Promise.withResolvers<Response>();
  const metadata = Promise.withResolvers<Response>();
  const started = [
    Promise.withResolvers<void>(),
    Promise.withResolvers<void>(),
    Promise.withResolvers<void>(),
  ] as const;
  server.use(
    http.get("*/api/posts/:id/comments", () => {
      started[0].resolve();
      return comments.promise;
    }),
    http.get("*/api/posts/:id", () => {
      started[1].resolve();
      return post.promise;
    }),
    http.get("*/api/subreddits/:subreddit", () => {
      started[2].resolve();
      return metadata.promise;
    }),
  );
  renderPath();
  await Promise.all(started.map((value) => value.promise));

  await act(async () => {
    comments.resolve(HttpResponse.json(COMMENTS_RESPONSE));
  });

  const discussion = screen.getByRole("region", { name: "Comments" });
  expect(
    await within(discussion).findByText(COMMENTS_RESPONSE.items[0]!.comment.bodyMarkdown),
  ).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Post" })).getByRole("status"),
  ).toHaveTextContent("Loading post");
  expect(within(screen.getByRole("complementary")).getByRole("status")).toBeVisible();

  await act(async () => {
    post.resolve(HttpResponse.json(POST_RESPONSE));
  });
  await screen.findByRole("heading", { name: POST_RESPONSE.title });
  expect(within(screen.getByRole("complementary")).getByRole("status")).toBeVisible();
  await act(async () => {
    metadata.resolve(HttpResponse.json(SUBREDDIT_RESPONSE));
  });
  await within(screen.getByRole("complementary")).findByText(SUBREDDIT_RESPONSE.title);
});

test("When comments are pending then a ready post should remain readable", async () => {
  const pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/posts/:id/comments", () => pending.promise));

  renderPath();

  expect(await screen.findByRole("heading", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Comments" })).getByRole("status"),
  ).toHaveTextContent("Loading comments");

  await act(async () => {
    pending.resolve(HttpResponse.json(COMMENTS_RESPONSE));
  });

  expect(await screen.findByText(COMMENTS_RESPONSE.items[0]!.comment.bodyMarkdown)).toBeVisible();
});

test("When comments fail then their retry should not refetch the post or metadata", async () => {
  const user = userEvent.setup();
  let commentsRequests = 0;
  let postRequests = 0;
  let metadataRequests = 0;
  server.use(
    http.get("*/api/posts/:id/comments", () =>
      ++commentsRequests === 1
        ? HttpResponse.json({ status: 503 }, { status: 503 })
        : HttpResponse.json(COMMENTS_RESPONSE),
    ),
    http.get("*/api/posts/:id", () => {
      postRequests++;
      return HttpResponse.json(POST_RESPONSE);
    }),
    http.get("*/api/subreddits/:subreddit", () => {
      metadataRequests++;
      return HttpResponse.json(SUBREDDIT_RESPONSE);
    }),
  );
  renderPath();
  const discussion = await screen.findByRole("region", { name: "Comments" });
  expect(await within(discussion).findByRole("alert")).toHaveTextContent("Unable to load comments");
  await screen.findByRole("heading", { name: POST_RESPONSE.title });

  await user.click(within(discussion).getByRole("button", { name: "Retry" }));

  await within(discussion).findByText(COMMENTS_RESPONSE.items[0]!.comment.bodyMarkdown);
  expect(commentsRequests).toBe(2);
  expect(postRequests).toBe(1);
  expect(metadataRequests).toBe(1);
});

test("When the post fails then available comments should still render", async () => {
  server.use(
    http.get("*/api/posts/:id", () => HttpResponse.json({ status: 503 }, { status: 503 })),
  );

  renderPath();

  expect(
    await within(await screen.findByRole("region", { name: "Post" })).findByRole("alert"),
  ).toHaveTextContent("Unable to load post");
  expect(
    await within(screen.getByRole("region", { name: "Comments" })).findByText(
      COMMENTS_RESPONSE.items[0]!.comment.bodyMarkdown,
    ),
  ).toBeVisible();
});

test("When a comment is collapsed then its body and replies should hide while its header remains", async () => {
  const user = userEvent.setup();
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id/comments", () => {
      requests++;
      return HttpResponse.json({
        appliedSort: "best",
        items: [commentItem("parent", "Parent body", [commentItem("child", "Child body")])],
      });
    }),
  );
  renderPath();
  await screen.findByText("Child body");
  const toggle = screen.getByRole("button", { name: "Toggle comment by parent" });

  await user.click(toggle);

  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(screen.getByText("u/parent")).toBeVisible();
  expect(screen.getByText("Parent body")).not.toBeVisible();
  expect(screen.queryByRole("button", { name: "Toggle comment by child" })).not.toBeInTheDocument();

  await user.click(toggle);

  expect(screen.getByText("Parent body")).toBeVisible();
  expect(screen.getByText("Child body")).toBeVisible();
  expect(requests).toBe(1);
});

test("When a comment is initially collapsed then keyboard activation should reveal its body", async () => {
  const user = userEvent.setup();
  const item = commentItem("collapsed", "Hidden body");
  item.comment.initiallyCollapsed = true;
  server.use(
    http.get("*/api/posts/:id/comments", () =>
      HttpResponse.json({ appliedSort: "best", items: [item] }),
    ),
  );
  renderPath();
  const toggle = await screen.findByRole("button", { name: "Toggle comment by collapsed" });
  expect(toggle).toHaveAttribute("aria-expanded", "false");

  toggle.focus();
  await user.keyboard("{Enter}");

  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByText("Hidden body")).toBeVisible();
});

test("When two placeholders are expanded then their requests should be independent and replace only their own branch", async () => {
  const user = userEvent.setup();
  const first = Promise.withResolvers<Response>();
  const second = Promise.withResolvers<Response>();
  const firstStarted = Promise.withResolvers<void>();
  const secondStarted = Promise.withResolvers<void>();
  const requests: URL[] = [];
  const firstCursor = "opaque+/=& ?#";
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      const url = new URL(request.url);
      requests.push(url);
      const cursor = url.searchParams.get("cursor");
      if (cursor === firstCursor) {
        firstStarted.resolve();
        return first.promise;
      }
      if (cursor === "second") {
        secondStarted.resolve();
        return second.promise;
      }
      return HttpResponse.json({
        appliedSort: "top",
        items: [
          commentItem("first", "First root", [
            { kind: "more", cursor: firstCursor, countHint: 999 },
          ]),
          commentItem("second", "Second root", [
            { kind: "more", cursor: "second", countHint: null },
          ]),
        ],
      });
    }),
  );
  renderPath(`${POST_PATH}?sort=top`);
  await screen.findByText("First root");
  const [firstButton, secondButton] = screen.getAllByRole("button", { name: "Load more replies" });
  expect(requests).toHaveLength(1);

  await user.click(firstButton!);
  await user.click(secondButton!);
  await Promise.all([firstStarted.promise, secondStarted.promise]);
  await act(async () => {
    second.resolve(
      HttpResponse.json({
        appliedSort: "top",
        items: [commentItem("second-reply", "Second reply")],
      }),
    );
  });

  expect(await screen.findByText("Second reply")).toBeVisible();
  expect(screen.getByRole("button", { name: "Loading replies" })).toBeDisabled();
  expect(screen.getByText("First root")).toBeVisible();

  await act(async () => {
    first.resolve(
      HttpResponse.json({ appliedSort: "top", items: [commentItem("first-reply", "First reply")] }),
    );
  });

  expect(await screen.findByText("First reply")).toBeVisible();
  expect(screen.getByText("Second root")).toBeVisible();
  expect(requests.map((url) => url.searchParams.get("cursor"))).toEqual([
    null,
    firstCursor,
    "second",
  ]);
  expect(requests.every((url) => url.searchParams.get("sort") === "top")).toBe(true);

  await user.click(screen.getByRole("button", { name: "Toggle comment by first" }));
  await user.click(screen.getByRole("button", { name: "Toggle comment by first" }));

  expect(screen.getByText("First reply")).toBeVisible();
  expect(requests).toHaveLength(3);
});

test("When an expansion fails then retry should leave the rest of the discussion intact", async () => {
  const user = userEvent.setup();
  let expansionRequests = 0;
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      if (new URL(request.url).searchParams.has("cursor")) {
        return ++expansionRequests === 1
          ? HttpResponse.json({ status: 503 }, { status: 503 })
          : HttpResponse.json({ appliedSort: "best", items: [] });
      }
      return HttpResponse.json({
        appliedSort: "best",
        items: [
          commentItem("root", "Root remains"),
          { kind: "more", cursor: "retry", countHint: 4 },
        ],
      });
    }),
  );
  renderPath();
  await screen.findByRole("button", { name: "Load more replies" });

  await user.click(screen.getByRole("button", { name: "Load more replies" }));

  expect(await screen.findByRole("alert")).toHaveTextContent("Unable to load replies");
  expect(screen.getByText("Root remains")).toBeVisible();

  await user.click(screen.getByRole("button", { name: "Retry" }));

  expect(await screen.findByText("No more replies.")).toBeVisible();
  expect(expansionRequests).toBe(2);
});

test("When sort changes then only comments should refetch and pending old expansions should not appear", async () => {
  const user = userEvent.setup();
  const oldExpansion = Promise.withResolvers<Response>();
  const started = Promise.withResolvers<void>();
  const sorts: string[] = [];
  let postRequests = 0;
  let metadataRequests = 0;
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      const query = new URL(request.url).searchParams;
      if (query.has("cursor")) {
        started.resolve();
        return oldExpansion.promise;
      }
      const sort = query.get("sort")!;
      sorts.push(sort);
      return HttpResponse.json({
        appliedSort: sort,
        items: [
          commentItem("root", `${sort} result`),
          { kind: "more", cursor: `${sort}-cursor`, countHint: null },
        ],
      });
    }),
    http.get("*/api/posts/:id", () => {
      postRequests++;
      return HttpResponse.json(POST_RESPONSE);
    }),
    http.get("*/api/subreddits/:subreddit", () => {
      metadataRequests++;
      return HttpResponse.json(SUBREDDIT_RESPONSE);
    }),
  );
  const router = renderPath();
  await screen.findByText("best result");
  await user.click(screen.getByRole("button", { name: "Load more replies" }));
  await started.promise;

  await user.click(screen.getByRole("radio", { name: "New" }));
  await screen.findByText("new result");
  await act(async () => {
    oldExpansion.resolve(
      HttpResponse.json({ appliedSort: "best", items: [commentItem("stale", "Stale reply")] }),
    );
  });

  expect(router.state.location.search).toBe("?sort=new");
  expect(screen.queryByText("Stale reply")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Load more replies" })).toBeEnabled();
  expect(sorts).toEqual(["best", "new"]);
  expect(postRequests).toBe(1);
  expect(metadataRequests).toBe(1);

  await act(async () => {
    await router.navigate(-1);
  });

  expect(screen.getByText("best result")).toBeVisible();
  expect(screen.getByRole("radio", { name: "Best" })).toBeChecked();
  expect(sorts).toEqual(["best", "new"]);
});

test("When Continue thread is followed then the focused route should preserve sort and reuse the post and metadata", async () => {
  const user = userEvent.setup();
  const requests: string[] = [];
  const target = commentItem("target", "Target body");
  const sibling = commentItem("sibling", "Collapsed sibling body");
  sibling.comment.initiallyCollapsed = true;
  const ancestor = commentItem("ancestor", "Ancestor body", [
    target,
    sibling,
    { kind: "more", cursor: "unsupported", countHint: 3 },
    { kind: "continueThread", commentId: "another" },
  ]);
  ancestor.comment.initiallyCollapsed = true;
  target.comment.initiallyCollapsed = true;
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      requests.push(new URL(request.url).pathname);
      return HttpResponse.json({
        appliedSort: "top",
        items: [{ kind: "continueThread", commentId: "target" }],
      });
    }),
    http.get("*/api/comments/:id/thread", ({ request }) => {
      requests.push(new URL(request.url).pathname);
      expect(new URL(request.url).searchParams.get("sort")).toBe("top");
      return HttpResponse.json({
        postId: POST_RESPONSE.id,
        forest: { appliedSort: "top", items: [ancestor] },
      });
    }),
    http.get("*/api/posts/:id", ({ request }) => {
      requests.push(new URL(request.url).pathname);
      return HttpResponse.json(POST_RESPONSE);
    }),
    http.get("*/api/subreddits/:subreddit", ({ request }) => {
      requests.push(new URL(request.url).pathname);
      return HttpResponse.json(SUBREDDIT_RESPONSE);
    }),
  );
  const router = renderPath(`${POST_PATH}?sort=top`);
  const link = await screen.findByRole("link", { name: "Continue thread" });
  expect(link).toHaveAttribute("href", `${POST_PATH}/_/target?sort=top`);

  await user.click(link);

  expect(await screen.findByText("Target body")).toBeVisible();
  expect(screen.getByText("Ancestor body")).toBeVisible();
  expect(screen.getByRole("button", { name: "Toggle comment by sibling" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  expect(screen.getByText("Selected comment")).toBeVisible();
  expect(screen.getByRole("button", { name: "Load more replies" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  expect(screen.getByRole("button", { name: "Continue thread" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  expect(router.state.location.pathname).toBe(`${POST_PATH}/_/target`);
  const trigger = screen.getByRole("button", { name: "Continue thread" });

  await user.hover(trigger);

  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    "Further expansion in a focused thread is not supported yet.",
  );
  await user.click(trigger);
  expect(router.state.location.pathname).toBe(`${POST_PATH}/_/target`);
  await user.unhover(trigger);
  await user.click(screen.getByRole("link", { name: "View full discussion" }));

  expect(await screen.findByRole("link", { name: "Continue thread" })).toBeVisible();
  expect(router.state.location.pathname).toBe(POST_PATH);
  expect(router.state.location.search).toBe("?sort=top");
  expect(requests.filter((path) => path === `/api/posts/${POST_RESPONSE.id}`)).toHaveLength(1);
  expect(requests.filter((path) => path === "/api/subreddits/rust")).toHaveLength(1);
  expect(requests.filter((path) => path.endsWith("/comments"))).toHaveLength(1);
  expect(requests.filter((path) => path.endsWith("/thread"))).toHaveLength(1);
});

test("When an expansion has no action token then a disabled control should explain why on keyboard focus", async () => {
  const user = userEvent.setup();
  server.use(
    http.get("*/api/posts/:id/comments", () =>
      HttpResponse.json({
        appliedSort: "best",
        items: [{ kind: "more", cursor: null, countHint: 2 }],
      }),
    ),
  );
  renderPath();
  const trigger = await screen.findByRole("button", { name: "Load more replies" });

  await user.tab();
  act(() => trigger.focus());

  expect(screen.getByRole("button", { name: "Load more replies" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    "No expansion is available for this item.",
  );
});

test("When comment data is sparse deleted or removed then readable replies should remain available", async () => {
  const sparse = commentItem("sparse", "");
  Object.assign(sparse.comment, {
    id: null,
    author: null,
    createdAt: null,
    score: null,
    bodyMarkdown: null,
    isOp: null,
    isPinned: null,
    initiallyCollapsed: null,
  });
  const deleted = commentItem("deleted", "Do not display deleted body", [
    commentItem("reply", "Surviving reply"),
  ]);
  deleted.comment.state = "deleted";
  const removed = commentItem("removed", "Do not display removed body");
  removed.comment.state = "removed";
  const op = commentItem("op", "**Formatted** comment");
  Object.assign(op.comment, { score: 0, isOp: true, isPinned: true });
  server.use(
    http.get("*/api/posts/:id/comments", () =>
      HttpResponse.json({ appliedSort: null, items: [sparse, deleted, removed, op] }),
    ),
  );

  renderPath();

  const discussion = await screen.findByRole("region", { name: "Comments" });
  expect(await within(discussion).findByText("Unknown author")).toBeVisible();
  expect(within(discussion).getByText("Comment deleted.")).toBeVisible();
  expect(within(discussion).getByText("Comment removed.")).toBeVisible();
  expect(within(discussion).getByText("Surviving reply")).toBeVisible();
  expect(within(discussion).queryByText(/Do not display/)).toBeNull();
  expect(within(discussion).getByText("0 points")).toBeVisible();
  expect(within(discussion).getByText("OP")).toBeVisible();
  expect(within(discussion).getByText("Pinned")).toBeVisible();
  expect(within(discussion).getByText("Formatted").tagName).toBe("STRONG");
});

test("When the comment sort is unsupported then the post should still render without requesting comments", async () => {
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id/comments", () => {
      requests++;
      return HttpResponse.json(COMMENTS_RESPONSE);
    }),
  );

  renderPath(`${POST_PATH}?sort=invalid`);

  expect(await screen.findByRole("heading", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Comments" })).getByRole("alert"),
  ).toHaveTextContent("Unsupported comment sort");
  expect(requests).toBe(0);
});

test("When a focused thread is opened directly then it should load without requesting the full discussion or waiting for the post", async () => {
  const pending = Promise.withResolvers<Response>();
  let fullRequests = 0;
  const threadRequests: URL[] = [];
  server.use(
    http.get("*/api/posts/:id", () => pending.promise),
    http.get("*/api/posts/:id/comments", () => {
      fullRequests++;
      return HttpResponse.json(COMMENTS_RESPONSE);
    }),
    http.get("*/api/comments/:id/thread", ({ request }) => {
      threadRequests.push(new URL(request.url));
      return HttpResponse.json({
        postId: null,
        forest: { appliedSort: "new", items: [commentItem("target", "Direct thread")] },
      });
    }),
  );

  renderPath(`${POST_PATH}/old_title/target?sort=new`);

  expect(await screen.findByText("Direct thread")).toBeVisible();
  expect(screen.getByText("Selected comment")).toBeVisible();
  expect(within(screen.getByRole("region", { name: "Post" })).getByRole("status")).toBeVisible();
  expect(fullRequests).toBe(0);
  expect(threadRequests).toHaveLength(1);
  expect(threadRequests[0]?.pathname).toBe("/api/comments/target/thread");
  expect(threadRequests[0]?.searchParams.get("sort")).toBe("new");

  await act(async () => {
    pending.resolve(HttpResponse.json(POST_RESPONSE));
  });
  await screen.findByRole("heading", { name: POST_RESPONSE.title });
});

test("When an expansion returns another placeholder then it should wait for a separate click", async () => {
  const user = userEvent.setup();
  const cursors: Array<string | null> = [];
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      const cursor = new URL(request.url).searchParams.get("cursor");
      cursors.push(cursor);
      const items =
        cursor === "second"
          ? [commentItem("second-reply", "Second expansion")]
          : cursor === "first"
            ? [
                commentItem("first-reply", "First expansion"),
                { kind: "more", cursor: "second", countHint: null },
              ]
            : [{ kind: "more", cursor: "first", countHint: null }];
      return HttpResponse.json({ appliedSort: "best", items });
    }),
  );
  renderPath();
  await screen.findByRole("button", { name: "Load more replies" });

  await user.click(screen.getByRole("button", { name: "Load more replies" }));

  expect(await screen.findByText("First expansion")).toBeVisible();
  expect(cursors).toEqual([null, "first"]);

  await user.click(screen.getByRole("button", { name: "Load more replies" }));

  expect(await screen.findByText("Second expansion")).toBeVisible();
  expect(screen.getByText("First expansion")).toBeVisible();
  expect(cursors).toEqual([null, "first", "second"]);
});

test("When the initial discussion is empty then it should show an empty state without expansion controls", async () => {
  server.use(
    http.get("*/api/posts/:id/comments", () =>
      HttpResponse.json({ appliedSort: "best", items: [] }),
    ),
  );

  renderPath();

  const discussion = await screen.findByRole("region", { name: "Comments" });
  expect(await within(discussion).findByText("No comments")).toBeVisible();
  expect(within(discussion).queryByRole("button", { name: "Load more replies" })).toBeNull();
  expect(within(discussion).queryByRole("list")).toBeNull();
});

test("When comment sorting is pending then the old tree should preserve collapsed and expanded branches with stale actions disabled", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  const started = Promise.withResolvers<void>();
  const requests: URL[] = [];
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      const url = new URL(request.url);
      requests.push(url);
      if (url.searchParams.get("sort") === "new") {
        started.resolve();
        return pending.promise;
      }
      if (url.searchParams.get("cursor") === "loaded")
        return HttpResponse.json({
          appliedSort: "best",
          items: [commentItem("loaded", "Already loaded reply")],
        });
      return HttpResponse.json({
        appliedSort: "best",
        items: [
          commentItem("parent", "Parent body", [
            { kind: "more", cursor: "loaded", countHint: null },
          ]),
          commentItem("collapsed", "Collapsed body"),
          { kind: "more", cursor: "unused", countHint: null },
          { kind: "continueThread", commentId: "target" },
        ],
      });
    }),
  );
  const router = renderPath();
  await screen.findByText("Parent body");
  await user.click(screen.getAllByRole("button", { name: "Load more replies" })[0]!);
  const reply = await screen.findByText("Already loaded reply");
  const collapsed = screen.getByRole("button", { name: "Toggle comment by collapsed" });
  await user.click(collapsed);
  const discussion = screen.getByRole("region", { name: "Comments" });
  const tree = within(discussion).getAllByRole("list")[0];
  const region = tree!.closest("[aria-busy]");

  await user.click(screen.getByRole("radio", { name: "New" }));
  await started.promise;

  expect(router.state.location.search).toBe("?sort=new");
  expect(region).toHaveAttribute("aria-busy", "true");
  expect(region).toHaveClass("opacity-80");
  expect(within(discussion).getAllByRole("list")[0]).toBe(tree);
  expect(screen.getByText("Already loaded reply")).toBe(reply);
  expect(reply).toBeVisible();
  expect(screen.getByRole("button", { name: "Toggle comment by collapsed" })).toBe(collapsed);
  expect(collapsed).toHaveAttribute("aria-expanded", "false");
  expect(screen.getByText("Collapsed body")).not.toBeVisible();
  expect(within(discussion).queryByRole("status")).toBeNull();
  expect(within(discussion).queryByText("Applied sort: best")).toBeNull();
  const more = screen.getByRole("button", { name: "Load more replies" });
  expect(more).toBeDisabled();
  const continuation = screen.getByRole("link", { name: "Continue thread" });
  expect(continuation).toHaveAttribute("aria-disabled", "true");
  expect(continuation).not.toHaveAttribute("href");

  await user.click(more);
  await user.click(continuation);

  expect(
    requests.map((url) => [url.searchParams.get("sort"), url.searchParams.get("cursor")]),
  ).toEqual([
    ["best", null],
    ["best", "loaded"],
    ["new", null],
  ]);
  expect(router.state.location.pathname).toBe(POST_PATH);

  await act(async () => {
    pending.resolve(
      HttpResponse.json({ appliedSort: "new", items: [commentItem("new", "New comment result")] }),
    );
  });

  expect(await screen.findByText("New comment result")).toBeVisible();
  expect(screen.queryByText("Already loaded reply")).toBeNull();
  expect(region).toHaveAttribute("aria-busy", "false");
  expect(region).not.toHaveClass("opacity-80");
});

test("When a focused thread sort is pending then its existing collapse state should remain until replacement", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/comments/target/thread", ({ request }) =>
      new URL(request.url).searchParams.get("sort") === "new"
        ? pending.promise
        : HttpResponse.json({
            postId: POST_RESPONSE.id,
            forest: { appliedSort: "best", items: [commentItem("target", "Old thread result")] },
          }),
    ),
  );
  renderPath(`${POST_PATH}/_/target`);
  await screen.findByText("Old thread result");
  const toggle = screen.getByRole("button", { name: "Toggle comment by target" });
  await user.click(toggle);
  const region = toggle.closest("[aria-busy]");

  await user.click(screen.getByRole("radio", { name: "New" }));

  expect(region).toHaveAttribute("aria-busy", "true");
  expect(screen.getByRole("button", { name: "Toggle comment by target" })).toBe(toggle);
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(screen.getByText("Old thread result")).not.toBeVisible();
  expect(screen.getByText("Selected comment")).toBeVisible();

  await act(async () => {
    pending.resolve(
      HttpResponse.json({
        postId: POST_RESPONSE.id,
        forest: { appliedSort: "new", items: [commentItem("target", "New thread result")] },
      }),
    );
  });

  expect(await screen.findByText("New thread result")).toBeVisible();
  expect(region).toHaveAttribute("aria-busy", "false");
  expect(screen.queryByText("Old thread result")).toBeNull();
});

test.each([
  [
    "another post",
    POST_PATH,
    "/r/rust/comments/other",
    `*/api/posts/${POST_RESPONSE.id}/comments`,
    "*/api/posts/other/comments",
    false,
    false,
  ],
  [
    "another thread",
    `${POST_PATH}/_/first`,
    `${POST_PATH}/_/second`,
    "*/api/comments/first/thread",
    "*/api/comments/second/thread",
    true,
    true,
  ],
  [
    "a focused thread",
    POST_PATH,
    `${POST_PATH}/_/first`,
    `*/api/posts/${POST_RESPONSE.id}/comments`,
    "*/api/comments/first/thread",
    false,
    true,
  ],
  [
    "the full discussion",
    `${POST_PATH}/_/first`,
    POST_PATH,
    "*/api/comments/first/thread",
    `*/api/posts/${POST_RESPONSE.id}/comments`,
    true,
    false,
  ],
] as const)(
  "When navigating to %s then unrelated comments should not be retained",
  async (_, initialPath, nextPath, initialEndpoint, nextEndpoint, initialThread, nextThread) => {
    const pending = Promise.withResolvers<Response>();
    const forest = { appliedSort: "best", items: [commentItem("first", "Old scope content")] };
    server.use(
      http.get(nextEndpoint, () => pending.promise),
      http.get(initialEndpoint, () =>
        HttpResponse.json(initialThread ? { postId: POST_RESPONSE.id, forest } : forest),
      ),
    );
    const router = renderPath(initialPath);
    await screen.findByText("Old scope content");

    await act(async () => {
      await router.navigate(nextPath);
    });

    const discussion = screen.getByRole("region", { name: "Comments" });
    expect(within(discussion).getByRole("status")).toHaveTextContent("Loading comments");
    expect(within(discussion).queryByText("Old scope content")).toBeNull();

    const nextForest = { appliedSort: "best", items: [commentItem("second", "New scope content")] };
    await act(async () => {
      pending.resolve(
        HttpResponse.json(
          nextThread ? { postId: POST_RESPONSE.id, forest: nextForest } : nextForest,
        ),
      );
    });

    expect(await within(discussion).findByText("New scope content")).toBeVisible();
  },
);

test("When sorting supersedes an in-flight expansion then its late response should not change the retained tree", async () => {
  const user = userEvent.setup();
  const expansion = Promise.withResolvers<Response>();
  const replacement = Promise.withResolvers<Response>();
  const expansionStarted = Promise.withResolvers<void>();
  const expansionAborted = Promise.withResolvers<void>();
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      const query = new URL(request.url).searchParams;
      if (query.has("cursor")) {
        request.signal.addEventListener("abort", () => expansionAborted.resolve());
        expansionStarted.resolve();
        return expansion.promise;
      }
      if (query.get("sort") === "new") return replacement.promise;
      return HttpResponse.json({
        appliedSort: "best",
        items: [
          commentItem("root", "Retained root"),
          { kind: "more", cursor: "old", countHint: null },
        ],
      });
    }),
  );
  renderPath();
  await screen.findByText("Retained root");
  await user.click(screen.getByRole("button", { name: "Load more replies" }));
  await expansionStarted.promise;

  await user.click(screen.getByRole("radio", { name: "New" }));
  await expansionAborted.promise;
  await act(async () => {
    expansion.resolve(
      HttpResponse.json({ appliedSort: "best", items: [commentItem("late", "Late expansion")] }),
    );
  });

  expect(screen.getByText("Retained root")).toBeVisible();
  expect(screen.queryByText("Late expansion")).toBeNull();
  expect(screen.getByRole("button", { name: "Load more replies" })).toBeDisabled();

  await act(async () => {
    replacement.resolve(
      HttpResponse.json({ appliedSort: "new", items: [commentItem("new", "Replacement root")] }),
    );
  });

  expect(await screen.findByText("Replacement root")).toBeVisible();
  expect(screen.queryByText("Late expansion")).toBeNull();
});

test("When a replacement discussion fails then retry should recover the requested sort", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  let newRequests = 0;
  server.use(
    http.get("*/api/posts/:id/comments", ({ request }) => {
      if (new URL(request.url).searchParams.get("sort") === "new")
        return ++newRequests === 1
          ? pending.promise
          : HttpResponse.json({
              appliedSort: "new",
              items: [commentItem("new", "Recovered comments")],
            });
      return HttpResponse.json({
        appliedSort: "best",
        items: [commentItem("old", "Old comments")],
      });
    }),
  );
  renderPath();
  await screen.findByText("Old comments");
  await user.click(screen.getByRole("radio", { name: "New" }));

  await act(async () => {
    pending.resolve(HttpResponse.json({ status: 503 }, { status: 503 }));
  });

  const discussion = screen.getByRole("region", { name: "Comments" });
  expect(await within(discussion).findByRole("alert")).toHaveTextContent("Unable to load comments");
  expect(within(discussion).queryByText("Old comments")).toBeNull();

  await user.click(within(discussion).getByRole("button", { name: "Retry" }));

  const result = await within(discussion).findByText("Recovered comments");
  expect(result.closest("[aria-busy]")).toHaveAttribute("aria-busy", "false");
  expect(within(discussion).getByRole("radio", { name: "New" })).toBeChecked();
  expect(newRequests).toBe(2);
});
