import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw/http";
import { expect, expectTypeOf, onTestFinished, test, vi } from "vitest";
import {
  COMMENTS_RESPONSE,
  COMMENT_THREAD_RESPONSE,
  POST_RESPONSE,
  SUBREDDIT_PAGE_RESPONSE,
} from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { createQueryClient } from "@/app/query-client";
import * as generatedQueries from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { getPostQueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type {
  CommentForest,
  Post,
  Problem,
  SubredditPage,
} from "@/clients/lurklet-api/generated/types.gen";
import { apiQueryOptions } from "@/clients/lurklet-api/query-options";
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
          apiQueryOptions.listSubredditPosts({
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
  server.use(
    http.get("*/api/comments/:id/thread", () =>
      HttpResponse.json(COMMENT_THREAD_RESPONSE, { headers: { Age: "3600" } }),
    ),
  );
  const now = Date.now();
  const clock = vi.spyOn(Date, "now").mockReturnValue(now);
  onTestFinished(() => clock.mockRestore());
  const queryClient = createQueryClient();
  const options = apiQueryOptions.getCommentThread({
    path: { id: COMMENTS_RESPONSE.items[0]!.comment.id },
    query: { sort: "new" },
  });

  const { result } = renderQuery(
    () => useJsonQuery({ ...options, select: (value) => value.data.forest }),
    undefined,
    queryClient,
  );
  await waitFor(() => expect(result.current.query.isSuccess).toBe(true));

  expect(result.current.query.data).toEqual(COMMENT_THREAD_RESPONSE.forest);
  expect(queryClient.getQueryData(options.queryKey)?.data).toEqual(COMMENT_THREAD_RESPONSE);
  expect(result.current.fetchedAt).toBe(now - 3_600_000);
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

test("When generated endpoint metadata changes then the JSON link should use the generated path", async () => {
  const options = { path: { id: POST_RESPONSE.id } };
  const generated = generatedQueries.getPostOptions(options);
  const metadata = vi.spyOn(generatedQueries, "getPostOptions").mockReturnValue({
    ...generated,
    meta: { apiPath: "/api/v2/posts/{id}" },
  });
  onTestFinished(() => metadata.mockRestore());

  const { result } = renderQuery(() => useJsonQuery(apiQueryOptions.getPost(options)), undefined);
  await waitFor(() => expect(result.current.query.isSuccess).toBe(true));

  expect(result.current.jsonHref).toBe(
    new URL(`/api/v2/posts/${POST_RESPONSE.id}`, location.origin).href,
  );
});

test("When a query loads refetches and fails then its JSON link should remain available", async () => {
  let pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/posts/:id", () => pending.promise));
  const href = new URL(`/api/posts/${POST_RESPONSE.id}`, location.origin).href;

  const { result } = renderQuery(
    () => useJsonQuery(apiQueryOptions.getPost({ path: { id: POST_RESPONSE.id } })),
    undefined,
  );

  expect(result.current.query.isPending).toBe(true);
  expect(result.current.query.data).toBeUndefined();
  if (result.current.query.isPending) {
    expectTypeOf(result.current.query.data).toEqualTypeOf<undefined>();
  }
  expect(result.current.jsonHref).toBe(href);
  expect(result.current.fetchedAt).toBeUndefined();

  await act(async () =>
    pending.resolve(HttpResponse.json(POST_RESPONSE, { headers: { Age: "60" } })),
  );
  await waitFor(() => expect(result.current.query.isSuccess).toBe(true));
  expect(result.current.jsonHref).toBe(href);
  expect(result.current.query.isFetching).toBe(false);
  const fetchedAt = result.current.fetchedAt;
  expect(fetchedAt).toEqual(expect.any(Number));
  pending = Promise.withResolvers<Response>();

  act(() => {
    void result.current.query.refetch();
  });
  await waitFor(() => expect(result.current.query.isFetching).toBe(true));

  expect(result.current.jsonHref).toBe(href);
  expect(result.current.fetchedAt).toBe(fetchedAt);

  await act(async () => pending.resolve(HttpResponse.json({ status: 503 }, { status: 503 })));
  await waitFor(() => expect(result.current.query.isError).toBe(true));

  expect(result.current.jsonHref).toBe(href);
  expect(result.current.fetchedAt).toBeUndefined();
  expect(result.current.query.data).toEqual(POST_RESPONSE);
  if (result.current.query.isRefetchError) {
    expectTypeOf(result.current.query.data).toEqualTypeOf<Post>();
    expectTypeOf(result.current.query.error).toEqualTypeOf<Problem>();
  }
});

test.each(["boolean", "callback"])(
  "When a cached query uses a disabled %s condition then its JSON link should wait until enabled",
  (condition) => {
    const queryClient = createQueryClient();
    queryClient.setQueryData(getPostQueryKey({ path: { id: POST_RESPONSE.id } }), {
      data: POST_RESPONSE,
      fetchedAt: undefined,
    });

    const { result, rerender } = renderQuery(
      (isEnabled) =>
        useJsonQuery({
          ...apiQueryOptions.getPost({ path: { id: POST_RESPONSE.id } }),
          enabled:
            condition === "callback"
              ? (query) => isEnabled && query.state.data?.data.id === POST_RESPONSE.id
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

test("When identical data is refetched then its age should update independently and disappear when the header is absent", async () => {
  const now = Date.now();
  const clock = vi.spyOn(Date, "now").mockReturnValue(now);
  onTestFinished(() => clock.mockRestore());
  let age: string | undefined = "3060";
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id", () => {
      requests++;
      return HttpResponse.json(POST_RESPONSE, { headers: age === undefined ? {} : { Age: age } });
    }),
  );
  const queryClient = createQueryClient();
  const options = apiQueryOptions.getPost({ path: { id: POST_RESPONSE.id } });
  const { result, unmount } = renderQuery(() => useJsonQuery(options), undefined, queryClient);
  await waitFor(() => expect(result.current.query.isSuccess).toBe(true));
  const data = result.current.query.data;

  expect(result.current.fetchedAt).toBe(now - 3_060_000);

  age = "0";
  await act(async () => {
    const refreshed = await result.current.query.refetch();

    expect(refreshed.data).toBe(data);
    expectTypeOf(refreshed.data).toEqualTypeOf<Post | undefined>();
    if (refreshed.isSuccess) {
      expectTypeOf(refreshed.data).toEqualTypeOf<Post>();
    }
  });

  await waitFor(() => expect(result.current.fetchedAt).toBe(now));
  expect(result.current.query.data).toBe(data);
  unmount();
  clock.mockReturnValue(now + 60_000);

  const cached = renderQuery(() => useJsonQuery(options), undefined, queryClient);

  expect(cached.result.current.fetchedAt).toBe(now);
  expect(requests).toBe(2);

  age = undefined;
  await act(async () => {
    await cached.result.current.query.refetch();
  });

  await waitFor(() => expect(cached.result.current.fetchedAt).toBeUndefined());
  expect(cached.result.current.query.data).toBe(data);
  expect(requests).toBe(3);
});
