import { cn } from "cn";
import { Link } from "react-router";
import type { CommentItem } from "@/clients/lurklet-api/generated/types.gen";
import { Button, buttonVariants } from "@/components/ui/button";
import { Tooltip, TooltipTrigger } from "@/components/ui/tooltip";
import type { CommentContext } from "@/features/comments/types";
import { CommentMore } from "./CommentMore";
import { CommentRow } from "./CommentRow";

export function CommentList({
  items,
  context,
  depth = 0,
}: {
  items: CommentItem[];
  context: CommentContext;
  depth?: number;
}) {
  return (
    <ol className="flex min-w-0 flex-col gap-4">
      {items.map((item, index) => {
        const id =
          item.kind === "comment"
            ? item.comment.id
            : item.kind === "more"
              ? item.cursor
              : item.commentId;
        const label = item.kind === "more" ? "Load more replies" : "Continue thread";
        return (
          <li key={`${item.kind}:${id ?? index}`} className="min-w-0">
            {item.kind === "comment" ? (
              <CommentRow item={item} context={context} depth={depth} />
            ) : context.focusedCommentId !== null || id === null ? (
              <TooltipTrigger>
                <Button
                  variant="outline"
                  size="sm"
                  aria-disabled="true"
                  className="cursor-not-allowed opacity-50"
                >
                  {label}
                </Button>
                <Tooltip>
                  {context.focusedCommentId !== null
                    ? "Further expansion in a focused thread is not supported yet."
                    : "No expansion is available for this item."}
                </Tooltip>
              </TooltipTrigger>
            ) : item.kind === "more" ? (
              <CommentMore cursor={item.cursor!} context={context} depth={depth} />
            ) : context.isUpdating ? (
              <span
                role="link"
                aria-disabled="true"
                className={cn(
                  buttonVariants({ variant: "outline", size: "sm" }),
                  "cursor-not-allowed opacity-50",
                )}
              >
                Continue thread
              </span>
            ) : (
              <Link
                className={cn(buttonVariants({ variant: "outline", size: "sm" }))}
                to={{
                  pathname: `/r/${encodeURIComponent(context.subreddit)}/comments/${encodeURIComponent(context.postId)}/${encodeURIComponent(context.slug)}/${encodeURIComponent(item.commentId!)}`,
                  search: new URLSearchParams({ sort: context.sort }).toString(),
                }}
              >
                Continue thread
              </Link>
            )}
          </li>
        );
      })}
    </ol>
  );
}
