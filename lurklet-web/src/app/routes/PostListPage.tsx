import { useLoaderData, useParams } from "react-router";
import { listSubredditPostsOptions } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type { SubredditPage } from "@/clients/lurklet-api/generated/types.gen";
import { PaneMenu } from "@/components/PaneMenu";
import { PendingRegion } from "@/components/PendingRegion";
import type { postListLoader } from "@/features/posts/api/post-list-loader";
import { PostList } from "@/features/posts/components/PostList";
import { PostListControls } from "@/features/posts/components/PostListControls";
import { PostListError } from "@/features/posts/components/PostListError";
import { PostListLoading } from "@/features/posts/components/PostListLoading";
import { PostListPagination } from "@/features/posts/components/PostListPagination";
import { useJsonQuery } from "@/hooks/useJsonQuery";
import { usePageTitle } from "@/hooks/usePageTitle";
import { retainQueryView, type RetainedQueryView } from "@/utils/retain-query-view";

export function PostListPage() {
  const { subreddit } = useParams();
  usePageTitle(`r/${subreddit}`);
  const { options, sort, timeRange } = useLoaderData<typeof postListLoader>();
  const { query: page, jsonHref } = useJsonQuery({
    ...listSubredditPostsOptions(options),
    placeholderData: (previous, previousQuery) =>
      previous && previousQuery && previousQuery.queryKey[0].path?.subreddit === subreddit
        ? retainQueryView(previous, {
            hasPreviousPages: Boolean(previousQuery.queryKey[0].query?.after),
          })
        : undefined,
  });
  const displayed: RetainedQueryView<SubredditPage, { hasPreviousPages: boolean }> | undefined =
    page.data;

  return (
    <div className="relative flex flex-col gap-6">
      <h1 className="pr-10 text-2xl font-semibold wrap-anywhere">r/{subreddit}</h1>
      <PaneMenu jsonHref={jsonHref} label="Listing actions" />
      <PostListControls sort={sort} {...(timeRange ? { timeRange } : {})} />
      <PendingRegion
        pending={page.isPlaceholderData || (page.isFetching && page.data !== undefined)}
        className="flex flex-col gap-6"
      >
        {page.isPending ? (
          <PostListLoading />
        ) : page.isError ? (
          <PostListError
            error={page.error}
            onRetry={() => {
              void page.refetch();
            }}
          />
        ) : (
          <PostList subreddit={subreddit!} posts={page.data.posts} />
        )}
        <PostListPagination
          nextCursor={page.isSuccess ? page.data.nextCursor : null}
          hasPreviousPages={
            displayed?.__retainedContext?.hasPreviousPages ?? Boolean(options.query.after)
          }
          isDisabled={page.isPlaceholderData}
        />
      </PendingRegion>
    </div>
  );
}
