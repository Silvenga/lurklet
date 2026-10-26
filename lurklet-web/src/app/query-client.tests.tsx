import { focusManager, onlineManager, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw/http";
import { expect, onTestFinished, test, vi } from "vitest";
import { POST_RESPONSE, SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import {
  getPostOptions,
  listSubredditPostsOptions,
} from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { createQueryClient } from "./query-client";

test("When unrelated queries are mounted then each should finish without waiting for the other", async () => {
  const post = Promise.withResolvers<Response>();
  const posts = Promise.withResolvers<Response>();
  const postStarted = Promise.withResolvers<void>();
  const postsStarted = Promise.withResolvers<void>();
  server.use(
    http.get("*/api/posts/:id", () => {
      postStarted.resolve();
      return post.promise;
    }),
    http.get("*/api/subreddits/:subreddit/posts", () => {
      postsStarted.resolve();
      return posts.promise;
    }),
  );
  const queryClient = createQueryClient();
  const { result, unmount } = renderHook(
    () => ({
      post: useQuery(getPostOptions({ path: { id: POST_RESPONSE.id } })),
      posts: useQuery(
        listSubredditPostsOptions({ path: { subreddit: "rust" }, query: { sort: "hot" } }),
      ),
    }),
    {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      ),
    },
  );
  onTestFinished(() => {
    unmount();
    queryClient.clear();
  });
  await Promise.all([postStarted.promise, postsStarted.promise]);

  await act(async () => {
    post.resolve(HttpResponse.json(POST_RESPONSE));
  });

  await waitFor(() => expect(result.current.post.isSuccess).toBe(true));
  expect(result.current.posts.isPending).toBe(true);
  expect(result.current.post.data).toEqual(POST_RESPONSE);

  await act(async () => {
    posts.resolve(HttpResponse.json(SUBREDDIT_PAGE_RESPONSE));
  });

  await waitFor(() => expect(result.current.posts.isSuccess).toBe(true));
  expect(result.current.posts.data).toEqual(SUBREDDIT_PAGE_RESPONSE);
});

test("When multiple regions request the same query then they should share one request", async () => {
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id", () => {
      requests++;
      return HttpResponse.json(POST_RESPONSE);
    }),
  );
  const queryClient = createQueryClient();
  const { result, unmount } = renderHook(
    () => [
      useQuery(getPostOptions({ path: { id: POST_RESPONSE.id } })),
      useQuery(getPostOptions({ path: { id: POST_RESPONSE.id } })),
    ],
    {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      ),
    },
  );
  onTestFinished(() => {
    unmount();
    queryClient.clear();
  });

  await waitFor(() => expect(result.current.every((query) => query.isSuccess)).toBe(true));

  expect(requests).toBe(1);
  expect(result.current[0]?.data).toBe(result.current[1]?.data);
});

test("When a query fails then it should expose the error without automatic retries", async () => {
  let requests = 0;
  const problem = { type: "about:blank", title: "Unavailable", status: 503 };
  server.use(
    http.get("*/api/posts/:id", () => {
      requests++;
      return HttpResponse.json(problem, { status: 503 });
    }),
  );
  const queryClient = createQueryClient();
  onTestFinished(() => queryClient.clear());

  const result = queryClient.fetchQuery(getPostOptions({ path: { id: POST_RESPONSE.id } }));

  await expect(result).rejects.toEqual(problem);
  expect(requests).toBe(1);
});

test("When focus or connectivity returns then even a stale query should not refetch automatically", async () => {
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id", () => {
      requests++;
      return HttpResponse.json(POST_RESPONSE);
    }),
  );
  const queryClient = createQueryClient();
  const options = getPostOptions({ path: { id: POST_RESPONSE.id } });
  const { result, unmount } = renderHook(() => useQuery(options), {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
  onTestFinished(() => {
    unmount();
    queryClient.clear();
    focusManager.setFocused(undefined);
    onlineManager.setOnline(true);
  });
  await waitFor(() => expect(result.current.isSuccess).toBe(true));
  await act(async () => {
    await queryClient.invalidateQueries({ queryKey: options.queryKey, refetchType: "none" });
  });

  await act(async () => {
    focusManager.setFocused(false);
    onlineManager.setOnline(false);
    focusManager.setFocused(true);
    onlineManager.setOnline(true);
  });

  expect(result.current.isFetching).toBe(false);
  expect(requests).toBe(1);
});

test("When a query becomes inactive then it should remain reusable until five minutes have elapsed", async () => {
  vi.useFakeTimers();
  const queryClient = createQueryClient();
  onTestFinished(() => {
    queryClient.clear();
    vi.useRealTimers();
  });
  const queryFn = vi.fn(async () => POST_RESPONSE);
  const options = { queryKey: ["retention"], queryFn };
  await queryClient.fetchQuery(options);

  await vi.advanceTimersByTimeAsync(5 * 60 * 1000 - 1);
  const cached = await queryClient.fetchQuery(options);

  expect(cached).toEqual(POST_RESPONSE);
  expect(queryFn).toHaveBeenCalledTimes(1);

  await vi.advanceTimersByTimeAsync(1);

  expect(queryClient.getQueryData(options.queryKey)).toBeUndefined();

  await queryClient.fetchQuery(options);

  expect(queryFn).toHaveBeenCalledTimes(2);
});

test("When an active query remains mounted then it should not poll or expire on a timer", async () => {
  vi.useFakeTimers();
  const queryClient = createQueryClient();
  const queryFn = vi.fn(async () => POST_RESPONSE);
  const options = { queryKey: ["no-polling"], queryFn };
  await queryClient.fetchQuery(options);
  const { result, unmount } = renderHook(() => useQuery(options), {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
  onTestFinished(() => {
    unmount();
    queryClient.clear();
    vi.useRealTimers();
  });

  await act(async () => {
    await vi.advanceTimersByTimeAsync(10 * 60 * 1000);
  });

  expect(queryFn).toHaveBeenCalledTimes(1);
  expect(result.current.data).toEqual(POST_RESPONSE);
  expect(result.current.isStale).toBe(false);
});
