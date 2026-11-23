import { cn } from "cn";
import { Link, useParams, useSearchParams } from "react-router";
import type { CommentForest, CommentSort } from "@/clients/lurklet-api/generated/types.gen";
import { apiQueryOptions } from "@/clients/lurklet-api/query-options";
import { EmptyResult } from "@/components/EmptyResult";
import { PaneMenu } from "@/components/PaneMenu";
import { PendingRegion } from "@/components/PendingRegion";
import { RequestNotice } from "@/components/RequestNotice";
import { buttonVariants } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useJsonQuery } from "@/hooks/useJsonQuery";
import { retainQueryView, type RetainedQueryView } from "@/utils/retain-query-view";
import { CommentList } from "./CommentList";

export function CommentDiscussion() {
  const { subreddit, postId, slug, commentId } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  // Forward bookmarked sort values unchanged. The API validates them.
  const sort = (searchParams.get("sort") ?? "best") as CommentSort;
  const comments = useJsonQuery({
    ...apiQueryOptions.getPostComments({ path: { id: postId! }, query: { sort } }),
    enabled: !commentId,
    placeholderData: (previous, previousQuery) =>
      previous && previousQuery && previousQuery.queryKey[0].path?.id === postId
        ? retainQueryView(previous, previousQuery.queryKey[0].query?.sort ?? "best")
        : undefined,
  });
  const thread = useJsonQuery({
    ...apiQueryOptions.getCommentThread({ path: { id: commentId ?? "" }, query: { sort } }),
    enabled: Boolean(commentId),
    placeholderData: (previous, previousQuery) =>
      previous && previousQuery && previousQuery.queryKey[0].path?.id === commentId
        ? {
            ...previous,
            forest: retainQueryView(
              previous.forest,
              previousQuery.queryKey[0].query?.sort ?? "best",
            ),
          }
        : undefined,
  });
  const { query, jsonHref, fetchedAt } = commentId ? thread : comments;
  const displayed: RetainedQueryView<CommentForest, CommentSort> | undefined =
    query.data && ("forest" in query.data ? query.data.forest : query.data);
  const displayedSort = displayed?.__retainedContext ?? sort;

  return (
    <section aria-label="Comments" className="relative flex min-w-0 flex-col gap-4">
      <PaneMenu
        jsonHref={jsonHref}
        fetchedAt={fetchedAt}
        label={commentId ? "Comment thread actions" : "Comments actions"}
      />
      <div className="flex flex-wrap items-center justify-between gap-3 pr-10">
        <h2 className="text-xl font-semibold">{commentId ? "Comment thread" : "Comments"}</h2>
        {commentId && (
          <Link
            className={cn(buttonVariants({ variant: "outline", size: "sm" }))}
            to={{
              pathname: `/r/${encodeURIComponent(subreddit!)}/comments/${encodeURIComponent(postId!)}`,
              search: new URLSearchParams({ sort }).toString(),
            }}
          >
            View full discussion
          </Link>
        )}
      </div>
      <ToggleGroup
        aria-label="Comment sort"
        variant="outline"
        spacing={0}
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={[sort]}
        onSelectionChange={(keys) => {
          const nextSort = String([...keys][0]);
          if (nextSort === sort) return;
          const next = new URLSearchParams(searchParams);
          next.set("sort", nextSort);
          void setSearchParams(next, { preventScrollReset: true });
        }}
      >
        <ToggleGroupItem id="best">Best</ToggleGroupItem>
        <ToggleGroupItem id="top">Top</ToggleGroupItem>
        <ToggleGroupItem id="new">New</ToggleGroupItem>
      </ToggleGroup>
      {query.isPending ? (
        <div role="status" className="flex flex-col gap-4">
          <span className="sr-only">Loading comments</span>
          <Skeleton className="h-20 w-full" />
          <Skeleton className="h-20 w-full" />
        </div>
      ) : query.isError ? (
        <RequestNotice subject={commentId ? "Comment thread" : "Comments"} query={query} />
      ) : (
        <PendingRegion
          pending={query.isPlaceholderData || query.isFetching}
          className="flex min-w-0 flex-col gap-4"
        >
          {displayed?.appliedSort && displayed.appliedSort !== displayedSort && (
            <p className="text-muted-foreground text-sm">Applied sort: {displayed.appliedSort}</p>
          )}
          {!displayed?.items.length ? (
            <EmptyResult>No comments.</EmptyResult>
          ) : (
            <CommentList
              key={JSON.stringify([postId, displayedSort, commentId])}
              items={displayed.items}
              context={{
                postId: postId!,
                subreddit: subreddit!,
                slug: slug ?? "_",
                sort: displayedSort,
                focusedCommentId: commentId ?? null,
                isUpdating: query.isPlaceholderData,
              }}
            />
          )}
        </PendingRegion>
      )}
    </section>
  );
}
