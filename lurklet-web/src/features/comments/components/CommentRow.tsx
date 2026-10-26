import { cn } from "cn";
import { ChevronRightIcon } from "lucide-react";
import type { CommentItem } from "@/clients/lurklet-api/generated/types.gen";
import { Markdown } from "@/components/Markdown";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import type { CommentContext } from "@/features/comments/types";
import { CommentList } from "./CommentList";

const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

type CommentNode = Extract<CommentItem, { kind: "comment" }>;

function containsComment(item: CommentNode, id: string): boolean {
  return (
    item.comment.id === id ||
    item.replies.some((reply) => reply.kind === "comment" && containsComment(reply, id))
  );
}

export function CommentRow({
  item,
  context,
  depth,
}: {
  item: CommentNode;
  context: CommentContext;
  depth: number;
}) {
  const { comment, replies } = item;
  const isFocused = context.focusedCommentId !== null && comment.id === context.focusedCommentId;

  return (
    <Collapsible
      defaultExpanded={
        comment.initiallyCollapsed !== true ||
        (context.focusedCommentId !== null && containsComment(item, context.focusedCommentId))
      }
      className="flex min-w-0 flex-col gap-2"
    >
      <div className="flex items-start gap-2">
        <Button
          slot="trigger"
          variant="ghost"
          size="icon-xs"
          aria-label={`Toggle comment by ${comment.author?.name ?? "unknown author"}`}
        >
          <ChevronRightIcon aria-hidden="true" className="group-aria-expanded/button:rotate-90" />
        </Button>
        <div className="text-muted-foreground flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-sm select-text">
          <span>{comment.author?.name ? `u/${comment.author.name}` : "Unknown author"}</span>
          {comment.createdAt && (
            <time dateTime={comment.createdAt}>
              {DATE_FORMAT.format(new Date(comment.createdAt))}
            </time>
          )}
          {comment.score !== null && (
            <span>
              {comment.score} {comment.score === 1 ? "point" : "points"}
            </span>
          )}
          {comment.isOp && <Badge variant="secondary">OP</Badge>}
          {comment.isPinned && <Badge variant="secondary">Pinned</Badge>}
          {isFocused && <Badge variant="outline">Selected comment</Badge>}
        </div>
      </div>
      <CollapsibleContent>
        <div className="flex min-w-0 flex-col gap-4">
          <div className="min-w-0 select-text">
            {comment.state !== "present" ? (
              <p className="text-muted-foreground text-sm">
                {comment.state === "deleted" ? "Comment deleted." : "Comment removed."}
              </p>
            ) : comment.bodyMarkdown ? (
              <Markdown className="prose-sm max-w-none overflow-x-auto wrap-anywhere">
                {comment.bodyMarkdown}
              </Markdown>
            ) : null}
          </div>
          {replies.length > 0 && (
            <div className={cn("min-w-0 border-l", depth < 4 && "pl-3")}>
              <CommentList items={replies} context={context} depth={depth + 1} />
            </div>
          )}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
