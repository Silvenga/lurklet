import { QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw/http";
import { createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test } from "vitest";
import {
  POST_RESPONSE,
  SUBREDDIT_PAGE_RESPONSE,
  SUBREDDIT_NEXT_PAGE_RESPONSE,
} from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { listSubredditPostsQueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { createQueryClient } from "./query-client";
import { APP_ROUTES } from "./router";

function renderPath(path: string, queryClient = createQueryClient()) {
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

test("When paging then the old rows and pagination should remain mounted until the new page arrives", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  let requests = 0;
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      requests++;
      return new URL(request.url).searchParams.has("after")
        ? pending.promise
        : HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  const queryClient = createQueryClient();
  const router = renderPath("/r/rust", queryClient);
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  const content = screen.getByRole("region", { name: "Page content" });
  const list = within(content).getByRole("list");
  const pendingRegion = list.closest("[aria-busy]");
  const thumbnailTitle = SUBREDDIT_PAGE_RESPONSE.posts[3]!.title;
  await user.click(screen.getByRole("button", { name: `Reveal thumbnail for ${thumbnailTitle}` }));
  const thumbnail = screen.getByRole("img", { name: `Thumbnail for ${thumbnailTitle}` });

  await user.click(screen.getByRole("link", { name: "Next" }));

  await waitFor(() => expect(pendingRegion).toHaveAttribute("aria-busy", "true"));

  expect(within(content).getByRole("list")).toBe(list);
  expect(screen.getByRole("img", { name: `Thumbnail for ${thumbnailTitle}` })).toBe(thumbnail);
  expect(
    screen.queryByRole("button", { name: `Reveal thumbnail for ${thumbnailTitle}` }),
  ).toBeNull();
  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(pendingRegion).toHaveAttribute("aria-busy", "true");
  expect(pendingRegion).toHaveClass("opacity-80");
  expect(within(content).queryByRole("status")).toBeNull();
  const next = screen.getByRole("link", { name: "Next" });
  expect(next).toHaveAttribute("aria-disabled", "true");
  expect(next).not.toHaveAttribute("href");
  expect(screen.queryByRole("link", { name: "First page" })).toBeNull();
  const location = router.state.location;

  await user.click(next);

  expect(router.state.location).toBe(location);
  expect(
    queryClient.getQueryData(
      listSubredditPostsQueryKey({ path: { subreddit: "rust" }, query: { sort: "hot" } }),
    ),
  ).toEqual(SUBREDDIT_PAGE_RESPONSE);

  await act(async () => {
    pending.resolve(HttpResponse.json(SUBREDDIT_NEXT_PAGE_RESPONSE));
  });

  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });
  expect(pendingRegion).toHaveAttribute("aria-busy", "false");
  expect(pendingRegion).not.toHaveClass("opacity-80");
  expect(screen.queryByRole("link", { name: "Next" })).toBeNull();

  await user.click(screen.getByRole("link", { name: "First page" }));

  expect(await screen.findByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    screen.getByRole("button", { name: `Reveal thumbnail for ${thumbnailTitle}` }),
  ).toBeVisible();
  expect(pendingRegion).toHaveAttribute("aria-busy", "false");
  expect(within(content).queryByRole("status")).toBeNull();
  expect(requests).toBe(2);
});

test("When changing sort from a final page then its First page control should remain in place but disabled", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) =>
      new URL(request.url).searchParams.get("sort") === "new"
        ? pending.promise
        : HttpResponse.json(SUBREDDIT_NEXT_PAGE_RESPONSE),
    ),
  );
  renderPath("/r/rust?after=last-page");
  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });

  await user.click(screen.getByRole("radio", { name: "New" }));

  await waitFor(() =>
    expect(screen.getByRole("link", { name: "First page" })).toHaveAttribute(
      "aria-disabled",
      "true",
    ),
  );

  expect(
    screen.getByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title }),
  ).toBeVisible();
  expect(screen.getByRole("link", { name: "First page" })).toHaveAttribute("aria-disabled", "true");
  expect(screen.getByRole("link", { name: "First page" })).not.toHaveAttribute("href");
  expect(screen.queryByRole("link", { name: "Next" })).toBeNull();

  await act(async () => {
    pending.resolve(HttpResponse.json(SUBREDDIT_PAGE_RESPONSE));
  });

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(screen.queryByRole("link", { name: "First page" })).toBeNull();
  expect(screen.getByRole("link", { name: "Next" })).toHaveAttribute(
    "href",
    expect.stringContaining("sort=new"),
  );
});

test("When switching subreddits then the old community's posts should not be retained", async () => {
  const pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/subreddits/typescript/posts", () => pending.promise));
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });

  await act(async () => {
    await router.navigate("/r/typescript");
  });

  const content = screen.getByRole("region", { name: "Page content" });
  expect(within(content).getByRole("status")).toHaveTextContent("Loading posts");
  expect(within(content).queryByRole("link", { name: POST_RESPONSE.title })).toBeNull();

  await act(async () => {
    pending.resolve(HttpResponse.json({ posts: [], nextCursor: null }));
  });

  expect(await within(content).findByText("No posts")).toBeVisible();
});

test("When filters change rapidly then only the latest response should replace the retained listing", async () => {
  const user = userEvent.setup();
  const newPage = Promise.withResolvers<Response>();
  const topPage = Promise.withResolvers<Response>();
  const newStarted = Promise.withResolvers<void>();
  const topStarted = Promise.withResolvers<void>();
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      const sort = new URL(request.url).searchParams.get("sort");
      if (sort === "new") {
        newStarted.resolve();
        return newPage.promise;
      }
      if (sort === "top") {
        topStarted.resolve();
        return topPage.promise;
      }
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  renderPath("/r/rust");
  const oldPost = await screen.findByRole("link", { name: POST_RESPONSE.title });

  await user.click(screen.getByRole("radio", { name: "New" }));
  await newStarted.promise;
  await user.click(screen.getByRole("radio", { name: "Top" }));
  await topStarted.promise;
  await act(async () => {
    newPage.resolve(
      HttpResponse.json({
        ...SUBREDDIT_PAGE_RESPONSE,
        posts: [{ ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, title: "Outdated result" }],
      }),
    );
  });

  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBe(oldPost);
  expect(screen.queryByRole("link", { name: "Outdated result" })).toBeNull();
  expect(oldPost.closest("[aria-busy]")).toHaveAttribute("aria-busy", "true");

  await act(async () => {
    topPage.resolve(
      HttpResponse.json({
        ...SUBREDDIT_PAGE_RESPONSE,
        posts: [{ ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, title: "Latest result" }],
      }),
    );
  });

  const latest = await screen.findByRole("link", { name: "Latest result" });
  expect(latest.closest("[aria-busy]")).toHaveAttribute("aria-busy", "false");
});

test("When a replacement listing fails then retry should load the requested sort rather than presenting the old result as current", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  let newRequests = 0;
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      if (new URL(request.url).searchParams.get("sort") === "new") {
        return ++newRequests === 1
          ? pending.promise
          : HttpResponse.json(SUBREDDIT_NEXT_PAGE_RESPONSE);
      }
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  await user.click(screen.getByRole("radio", { name: "New" }));

  await act(async () => {
    pending.resolve(HttpResponse.json({ status: 503 }, { status: 503 }));
  });

  const content = screen.getByRole("region", { name: "Page content" });
  expect(await within(content).findByRole("alert")).toHaveTextContent("Unable to load posts");
  expect(within(content).queryByRole("link", { name: POST_RESPONSE.title })).toBeNull();
  expect(within(content).queryByRole("link", { name: "Next" })).toBeNull();

  await user.click(within(content).getByRole("button", { name: "Retry" }));

  const result = await within(content).findByRole("link", {
    name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title,
  });
  expect(result.closest("[aria-busy]")).toHaveAttribute("aria-busy", "false");
  expect(screen.getByRole("radio", { name: "New" })).toBeChecked();
  expect(newRequests).toBe(2);
});

test("When the top time range changes then the existing posts should remain visible while the new range loads", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) =>
      new URL(request.url).searchParams.get("timeRange") === "month"
        ? pending.promise
        : HttpResponse.json(SUBREDDIT_PAGE_RESPONSE),
    ),
  );
  const router = renderPath("/r/rust?sort=top&t=week");
  const oldPost = await screen.findByRole("link", { name: POST_RESPONSE.title });

  await user.click(screen.getByRole("radio", { name: "Month" }));

  await waitFor(() => expect(oldPost.closest("[aria-busy]")).toHaveAttribute("aria-busy", "true"));

  expect(router.state.location.search).toBe("?sort=top&t=month");
  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBe(oldPost);
  expect(oldPost.closest("[aria-busy]")).toHaveAttribute("aria-busy", "true");
  expect(screen.getByRole("radio", { name: "Month" })).toBeChecked();

  await act(async () => {
    pending.resolve(HttpResponse.json(SUBREDDIT_NEXT_PAGE_RESPONSE));
  });

  const result = await screen.findByRole("link", {
    name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title,
  });
  expect(result.closest("[aria-busy]")).toHaveAttribute("aria-busy", "false");
});
