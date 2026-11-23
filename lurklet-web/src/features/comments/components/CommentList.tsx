import { cn } from "cn";
import type { ReactNode } from "react";
import { Link } from "react-router";
import type { CommentItem } from "@/clients/lurklet-api/generated/types.gen";
import { buttonVariants } from "@/components/ui/button";
import type { CommentContext } from "@/features/comments/types";
import { CommentMore } from "./CommentMore";
import { CommentRow } from "./CommentRow";

export function CommentList({ items, context }: { items: CommentItem[]; context: CommentContext }) {
  function renderItems(siblings: CommentItem[], isReply: boolean): ReactNode {
    return siblings.map((item, index) => {
      const id =
        item.kind === "comment"
          ? item.comment.id
          : item.kind === "more"
            ? item.cursor
            : item.commentId;
      const key = `${item.kind}:${id ?? index}`;
      const renderRow = (content: ReactNode) => (
        <li
          key={key}
          className={cn(
            "min-w-0 pb-4 last:pb-0",
            isReply && "flex border-l border-border last:border-l-transparent",
          )}
        >
          {isReply && (
            <span
              aria-hidden="true"
              className="border-border pointer-events-none -ml-px h-3 w-2 shrink-0 rounded-bl-md border-b border-l sm:w-4"
            />
          )}
          <div className="min-w-0 flex-1">{content}</div>
        </li>
      );

      if (item.kind === "comment") {
        return renderRow(
          <CommentRow item={item} focusedCommentId={context.focusedCommentId}>
            {item.replies.length > 0 && <ol>{renderItems(item.replies, true)}</ol>}
          </CommentRow>,
        );
      }

      if (context.focusedCommentId !== null || id === null) {
        return renderRow(
          <p className="text-muted-foreground text-sm/6">
            {context.focusedCommentId !== null
              ? "Open the full discussion to load more replies."
              : "Reddit provided no link to more replies."}
          </p>,
        );
      }

      if (item.kind === "more") {
        return (
          <CommentMore
            key={key}
            cursor={id}
            context={context}
            renderRow={renderRow}
            renderItems={(page) => renderItems(page, isReply)}
          />
        );
      }

      const linkClassName = buttonVariants({ variant: "outline", size: "sm" });
      return renderRow(
        context.isUpdating ? (
          <span
            role="link"
            aria-disabled="true"
            className={cn(linkClassName, "cursor-not-allowed opacity-50")}
          >
            Continue thread
          </span>
        ) : (
          <Link
            className={linkClassName}
            to={{
              pathname: `/r/${encodeURIComponent(context.subreddit)}/comments/${encodeURIComponent(context.postId)}/${encodeURIComponent(context.slug)}/${encodeURIComponent(id)}`,
              search: new URLSearchParams({ sort: context.sort }).toString(),
            }}
          >
            Continue thread
          </Link>
        ),
      );
    });
  }

  return <ol>{renderItems(items, false)}</ol>;
}
