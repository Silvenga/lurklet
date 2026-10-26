import { useQuery } from "@tanstack/react-query";
import { cn } from "cn";
import { Link, useParams, useSearchParams } from "react-router";
import {
  getCommentThreadOptions,
  getPostCommentsOptions,
} from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type { CommentForest, CommentSort } from "@/clients/lurklet-api/generated/types.gen";
import { PendingRegion } from "@/components/PendingRegion";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button, buttonVariants } from "@/components/ui/button";
import { Empty, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { retainQueryView, type RetainedQueryView } from "@/utils/retain-query-view";
import { CommentList } from "./CommentList";

export function CommentDiscussion() {
  const { subreddit, postId, slug, commentId } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const requestedSort = searchParams.get("sort") ?? "best";
  const isValidSort =
    requestedSort === "best" || requestedSort === "top" || requestedSort === "new";
  const sort = isValidSort ? requestedSort : "best";
  const comments = useQuery({
    ...getPostCommentsOptions({ path: { id: postId! }, query: { sort } }),
    enabled: isValidSort && !commentId,
    placeholderData: (previous, previousQuery) =>
      previous && previousQuery && previousQuery.queryKey[0].path?.id === postId
        ? retainQueryView(previous, previousQuery.queryKey[0].query?.sort ?? "best")
        : undefined,
  });
  const thread = useQuery({
    ...getCommentThreadOptions({ path: { id: commentId ?? "" }, query: { sort } }),
    enabled: isValidSort && Boolean(commentId),
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
    select: (value) => value.forest,
  });
  const query = commentId ? thread : comments;
  const displayed: RetainedQueryView<CommentForest, CommentSort> | undefined = query.data;
  const displayedSort = displayed?.__retainedContext ?? sort;

  return (
    <section aria-label="Comments" className="flex min-w-0 flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
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
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={isValidSort ? [sort] : []}
        onSelectionChange={(keys) => {
          const nextSort = String([...keys][0]);
          if (nextSort === requestedSort) return;
          const next = new URLSearchParams(searchParams);
          next.set("sort", nextSort);
          void setSearchParams(next);
        }}
      >
        <ToggleGroupItem id="best">Best</ToggleGroupItem>
        <ToggleGroupItem id="top">Top</ToggleGroupItem>
        <ToggleGroupItem id="new">New</ToggleGroupItem>
      </ToggleGroup>
      {!isValidSort ? (
        <Alert>
          <AlertTitle>Unsupported comment sort</AlertTitle>
          <AlertDescription>Choose Best, Top, or New.</AlertDescription>
        </Alert>
      ) : query.isPending ? (
        <div role="status" className="flex flex-col gap-4">
          <span className="sr-only">Loading comments</span>
          <Skeleton className="h-20 w-full" />
          <Skeleton className="h-20 w-full" />
        </div>
      ) : query.isError ? (
        <Alert variant="destructive">
          <AlertTitle>Unable to load comments</AlertTitle>
          <AlertDescription>Please try again.</AlertDescription>
          <Button
            variant="outline"
            size="sm"
            className="mt-2 w-fit"
            onPress={() => {
              void query.refetch();
            }}
          >
            Retry
          </Button>
        </Alert>
      ) : (
        <PendingRegion
          pending={query.isPlaceholderData || query.isFetching}
          className="flex min-w-0 flex-col gap-4"
        >
          {query.data.appliedSort && query.data.appliedSort !== displayedSort && (
            <p className="text-muted-foreground text-sm">Applied sort: {query.data.appliedSort}</p>
          )}
          {query.data.items.length === 0 ? (
            <Empty>
              <EmptyHeader>
                <EmptyTitle>No comments</EmptyTitle>
              </EmptyHeader>
            </Empty>
          ) : (
            <CommentList
              key={JSON.stringify([postId, displayedSort, commentId])}
              items={query.data.items}
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
