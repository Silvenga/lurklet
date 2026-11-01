import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw/http";
import { expect, expectTypeOf, onTestFinished, test } from "vitest";
import {
  COMMENTS_RESPONSE,
  COMMENT_THREAD_RESPONSE,
  POST_RESPONSE,
  SUBREDDIT_PAGE_RESPONSE,
} from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { createQueryClient } from "@/app/query-client";
import {
  getCommentThreadOptions,
  getPostOptions,
  getPostQueryKey,
  listSubredditPostsOptions,
} from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type {
  CommentForest,
  Problem,
  SubredditPage,
} from "@/clients/lurklet-api/generated/types.gen";
import { useJsonQuery } from "./useJsonQuery";

function renderQuery<TProps, TResult>(
  callback: (props: TProps) => TResult,
  initialProps: TProps,
  queryClient = createQueryClient(),
) {
  const rendered = renderHook(callback, {
    initialProps,
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
  onTestFinished(() => {
    rendered.unmount();
    queryClient.clear();
  });
  return rendered;
}

test.each([undefined, "https://api.example.test"])(
  "When the API origin is %s then the JSON link should match the encoded request without fetching again",
  async (baseUrl) => {
    const requests: string[] = [];
    server.use(
      http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
        requests.push(request.url);
        return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
      }),
    );
    const subreddit = "rust +/?#";
    const after = "opaque+/=& ?#";

    const { result, rerender } = renderQuery(
      () =>
        useJsonQuery(
          listSubredditPostsOptions({
            ...(baseUrl ? { baseUrl } : {}),
            path: { subreddit },
            query: { sort: "top", timeRange: "week", after },
          }),
        ),
      undefined,
    );
    await waitFor(() => expect(result.current.query.isSuccess).toBe(true));
    rerender();

    const url = new URL(result.current.jsonHref!);
    expect(url.origin).toBe(baseUrl ?? location.origin);
    expect(url.pathname).toBe(`/api/subreddits/${encodeURIComponent(subreddit)}/posts`);
    expect(Object.fromEntries(url.searchParams)).toEqual({ sort: "top", timeRange: "week", after });
    expect(requests).toEqual([result.current.jsonHref]);
    expect(result.current.query.data).toEqual(SUBREDDIT_PAGE_RESPONSE);
    expectTypeOf(result.current.query.data).toEqualTypeOf<SubredditPage | undefined>();
    expectTypeOf(result.current.query.error).toEqualTypeOf<Problem | null>();
  },
);

test("When a thread query selects its forest then the JSON link should still address the original response", async () => {
  const queryClient = createQueryClient();
  const options = getCommentThreadOptions({
    path: { id: COMMENTS_RESPONSE.items[0]!.comment.id },
    query: { sort: "new" },
  });

  const { result } = renderQuery(
    () => useJsonQuery({ ...options, select: (value) => value.forest }),
    undefined,
    queryClient,
  );
  await waitFor(() => expect(result.current.query.isSuccess).toBe(true));

  expect(result.current.query.data).toEqual(COMMENT_THREAD_RESPONSE.forest);
  expect(queryClient.getQueryData(options.queryKey)).toEqual(COMMENT_THREAD_RESPONSE);
  expect(result.current.jsonHref).toBe(
    new URL(
      `/api/comments/${COMMENTS_RESPONSE.items[0]!.comment.id}/thread?sort=new`,
      location.origin,
    ).href,
  );
  expectTypeOf(result.current.query.data).toEqualTypeOf<CommentForest | undefined>();
  expectTypeOf(result.current.query.error).toEqualTypeOf<Problem | null>();
  if (result.current.query.isSuccess) {
    expectTypeOf(result.current.query.data).toEqualTypeOf<CommentForest>();
  }
});

test("When a query loads refetches and fails then its JSON link should remain available", async () => {
  let pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/posts/:id", () => pending.promise));
  const href = new URL(`/api/posts/${POST_RESPONSE.id}`, location.origin).href;

  const { result } = renderQuery(
    () => useJsonQuery(getPostOptions({ path: { id: POST_RESPONSE.id } })),
    undefined,
  );

  expect(result.current.query.isPending).toBe(true);
  expect(result.current.jsonHref).toBe(href);

  await act(async () => pending.resolve(HttpResponse.json(POST_RESPONSE)));
  await waitFor(() => expect(result.current.query.isSuccess).toBe(true));
  expect(result.current.jsonHref).toBe(href);
  expect(result.current.query.isFetching).toBe(false);
  pending = Promise.withResolvers<Response>();

  act(() => {
    void result.current.query.refetch();
  });
  await waitFor(() => expect(result.current.query.isFetching).toBe(true));

  expect(result.current.jsonHref).toBe(href);

  await act(async () => pending.resolve(HttpResponse.json({ status: 503 }, { status: 503 })));
  await waitFor(() => expect(result.current.query.isError).toBe(true));

  expect(result.current.jsonHref).toBe(href);
});

test.each(["boolean", "callback"])(
  "When a cached query uses a disabled %s condition then its JSON link should wait until enabled",
  (condition) => {
    const queryClient = createQueryClient();
    queryClient.setQueryData(getPostQueryKey({ path: { id: POST_RESPONSE.id } }), POST_RESPONSE);

    const { result, rerender } = renderQuery(
      (isEnabled) =>
        useJsonQuery({
          ...getPostOptions({ path: { id: POST_RESPONSE.id } }),
          enabled:
            condition === "callback"
              ? (query) => isEnabled && query.state.data?.id === POST_RESPONSE.id
              : isEnabled,
        }),
      false,
      queryClient,
    );

    expect(result.current.query.isSuccess).toBe(true);
    expect(result.current.jsonHref).toBeUndefined();

    rerender(true);

    expect(result.current.jsonHref).toBe(
      new URL(`/api/posts/${POST_RESPONSE.id}`, location.origin).href,
    );
    expect(result.current.query.isFetching).toBe(false);
  },
);
