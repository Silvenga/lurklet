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
  Problem,
} from "./generated/types.gen";
import { withResponseAge, type AgedResponse } from "./with-response-age";

function responseOptions<T, Key extends QueryKey>({
  meta,
  ...options
}: {
  queryKey: Key;
  queryFn: QueryFunction<AgedResponse<T>, Key>;
  meta: QueryMeta | undefined;
}) {
  return queryOptions<AgedResponse<T>, Problem, T, Key>({
    ...options,
    ...(meta ? { meta } : {}),
    select: (response) => response.data,
  });
}

export const apiQueryOptions = {
  getPost: (options: Options<GetPostData>) =>
    responseOptions({
      queryKey: getPostQueryKey(options),
      queryFn: ({ signal }) => withResponseAge(getPost({ ...options, signal, throwOnError: true })),
      meta: getPostOptions(options).meta,
    }),

  getSubreddit: (options: Options<GetSubredditData>) =>
    responseOptions({
      queryKey: getSubredditQueryKey(options),
      queryFn: ({ signal }) =>
        withResponseAge(getSubreddit({ ...options, signal, throwOnError: true })),
      meta: getSubredditOptions(options).meta,
    }),

  listSubredditPosts: (options: Options<ListSubredditPostsData>) =>
    responseOptions({
      queryKey: listSubredditPostsQueryKey(options),
      queryFn: ({ signal }) =>
        withResponseAge(listSubredditPosts({ ...options, signal, throwOnError: true })),
      meta: listSubredditPostsOptions(options).meta,
    }),

  getPostComments: (options: Options<GetPostCommentsData>) =>
    responseOptions({
      queryKey: getPostCommentsQueryKey(options),
      queryFn: ({ signal }) =>
        withResponseAge(getPostComments({ ...options, signal, throwOnError: true })),
      meta: getPostCommentsOptions(options).meta,
    }),

  getCommentThread: (options: Options<GetCommentThreadData>) =>
    responseOptions({
      queryKey: getCommentThreadQueryKey(options),
      queryFn: ({ signal }) =>
        withResponseAge(getCommentThread({ ...options, signal, throwOnError: true })),
      meta: getCommentThreadOptions(options).meta,
    }),
};
