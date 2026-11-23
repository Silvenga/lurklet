import { cn } from "cn";
import { ChevronRightIcon } from "lucide-react";
import type { ReactNode } from "react";
import type { CommentItem } from "@/clients/lurklet-api/generated/types.gen";
import { DateAgo } from "@/components/DateAgo";
import { Markdown } from "@/components/Markdown";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";

type CommentNode = Extract<CommentItem, { kind: "comment" }>;

function containsComment(item: CommentNode, id: string): boolean {
  return (
    item.comment.id === id ||
    item.replies.some((reply) => reply.kind === "comment" && containsComment(reply, id))
  );
}

export function CommentRow({
  item,
  focusedCommentId,
  children,
}: {
  item: CommentNode;
  focusedCommentId: string | null;
  children: ReactNode;
}) {
  const { comment } = item;
  const isFocused = focusedCommentId !== null && comment.id === focusedCommentId;

  return (
    <Collapsible
      defaultExpanded={
        comment.initiallyCollapsed !== true ||
        (focusedCommentId !== null && containsComment(item, focusedCommentId))
      }
      className="min-w-0"
    >
      <div className="flex gap-2">
        <div className="flex shrink-0 flex-col">
          <Button
            slot="trigger"
            variant="ghost"
            size="icon-xs"
            className="peer"
            aria-label={`Toggle comment by ${comment.author?.name ?? "unknown author"}`}
          >
            <ChevronRightIcon aria-hidden="true" className="group-aria-expanded/button:rotate-90" />
          </Button>
          {children && (
            <span
              aria-hidden="true"
              className="ml-3 hidden flex-1 border-l peer-aria-expanded:block"
            />
          )}
        </div>
        <div className="text-muted-foreground flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-sm select-text">
          <span>{comment.author?.name ? `u/${comment.author.name}` : "Unknown author"}</span>
          {comment.createdAt && <DateAgo dateTime={comment.createdAt} />}
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
      <CollapsibleContent className="ml-3">
        <div
          className={cn("min-w-0 pt-2 pl-5 select-text", children && "border-l border-border pb-3")}
        >
          {comment.state !== "present" ? (
            <p className="text-muted-foreground text-sm">
              {comment.state === "deleted" ? "Comment deleted." : "Comment removed."}
            </p>
          ) : comment.markdown ? (
            <Markdown className="prose-sm max-w-none overflow-x-auto" value={comment.markdown} />
          ) : null}
        </div>
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}
