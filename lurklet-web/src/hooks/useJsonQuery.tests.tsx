import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw/http";
import { expect, expectTypeOf, onTestFinished, test } from "vitest";
import { POST_RESPONSE, SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { createQueryClient } from "@/app/query-client";
import { getPostQueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type {
  FetchedResponsePost,
  FetchedResponseSubredditPage,
} from "@/clients/lurklet-api/generated/types.gen";
import { apiQueryOptions } from "@/clients/lurklet-api/query-options";
import type { RequestFailure } from "@/clients/lurklet-api/request-failure";
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
    expectTypeOf(result.current.query.data).toEqualTypeOf<
      FetchedResponseSubredditPage | undefined
    >();
    expectTypeOf(result.current.query.error).toEqualTypeOf<RequestFailure | null>();
  },
);

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

  const post = { ...POST_RESPONSE, fetchedAt: "2026-11-01T12:00:00Z" };
  await act(async () => pending.resolve(HttpResponse.json(post)));
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
  expect(result.current.query.data).toEqual(post);
  if (result.current.query.isRefetchError) {
    expectTypeOf(result.current.query.data).toEqualTypeOf<FetchedResponsePost>();
    expectTypeOf(result.current.query.error).toEqualTypeOf<RequestFailure>();
  }
});

test.each(["boolean", "callback"])(
  "When a cached query uses a disabled %s condition then its JSON link should wait until enabled",
  (condition) => {
    const queryClient = createQueryClient();
    queryClient.setQueryData(getPostQueryKey({ path: { id: POST_RESPONSE.id } }), {
      ...POST_RESPONSE,
      fetchedAt: "2026-11-01T12:00:00Z",
    });

    const { result, rerender } = renderQuery(
      (isEnabled) =>
        useJsonQuery({
          ...apiQueryOptions.getPost({ path: { id: POST_RESPONSE.id } }),
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
