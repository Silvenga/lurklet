import {
  queryOptions,
  type QueryFunction,
  type QueryKey,
  type QueryMeta,
} from "@tanstack/react-query";
import {
  getCommentThreadOptions,
  getCommentThreadQueryKey,
  getPostCommentsOptions,
  getPostCommentsQueryKey,
  getPostOptions,
  getPostQueryKey,
  getSubredditOptions,
  getSubredditQueryKey,
  listSubredditPostsOptions,
  listSubredditPostsQueryKey,
} from "./generated/@tanstack/react-query.gen";
import {
  getCommentThread,
  getPost,
  getPostComments,
  getSubreddit,
  listSubredditPosts,
  type Options,
} from "./generated/sdk.gen";
import type {
  GetCommentThreadData,
  GetPostCommentsData,
  GetPostData,
  GetSubredditData,
  ListSubredditPostsData,
} from "./generated/types.gen";
import { RequestFailure } from "./request-failure";

async function responseData<T>(
  pending: Promise<{ data?: T; response?: Response; error?: unknown }>,
) {
  const { data, response, error } = await pending;
  if (data !== undefined && response) return data;
  if (error instanceof Error && error.name === "AbortError") throw error;
  throw new RequestFailure(response, error);
}

function responseOptions<T, Key extends QueryKey>({
  meta,
  ...options
}: {
  queryKey: Key;
  queryFn: QueryFunction<T, Key>;
  meta: QueryMeta | undefined;
}) {
  return queryOptions<T, RequestFailure, T, Key>({
    ...options,
    networkMode: "always",
    ...(meta ? { meta } : {}),
  });
}

export const apiQueryOptions = {
  getPost: (options: Options<GetPostData>) =>
    responseOptions({
      queryKey: getPostQueryKey(options),
      queryFn: ({ signal }) => responseData(getPost({ ...options, signal, throwOnError: false })),
      meta: getPostOptions(options).meta,
    }),

  getSubreddit: (options: Options<GetSubredditData>) =>
    responseOptions({
      queryKey: getSubredditQueryKey(options),
      queryFn: ({ signal }) =>
        responseData(getSubreddit({ ...options, signal, throwOnError: false })),
      meta: getSubredditOptions(options).meta,
    }),

  listSubredditPosts: (options: Options<ListSubredditPostsData>) =>
    responseOptions({
      queryKey: listSubredditPostsQueryKey(options),
      queryFn: ({ signal }) =>
        responseData(listSubredditPosts({ ...options, signal, throwOnError: false })),
      meta: listSubredditPostsOptions(options).meta,
    }),

  getPostComments: (options: Options<GetPostCommentsData>) =>
    responseOptions({
      queryKey: getPostCommentsQueryKey(options),
      queryFn: ({ signal }) =>
        responseData(getPostComments({ ...options, signal, throwOnError: false })),
      meta: getPostCommentsOptions(options).meta,
    }),

  getCommentThread: (options: Options<GetCommentThreadData>) =>
    responseOptions({
      queryKey: getCommentThreadQueryKey(options),
      queryFn: ({ signal }) =>
        responseData(getCommentThread({ ...options, signal, throwOnError: false })),
      meta: getCommentThreadOptions(options).meta,
    }),
};
