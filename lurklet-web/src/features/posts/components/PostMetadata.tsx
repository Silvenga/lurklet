import type { ListedPost } from "@/clients/lurklet-api/generated/types.gen";
import { DateAgo } from "@/components/DateAgo";

type PostMetadataProps = Pick<ListedPost, "authorName" | "createdAt" | "score" | "commentCount">;

export function PostMetadata({ authorName, createdAt, score, commentCount }: PostMetadataProps) {
  return (
    <p className="text-muted-foreground flex flex-wrap gap-x-3 gap-y-1 text-sm">
      {authorName !== null && <span>u/{authorName}</span>}
      {createdAt !== null && <DateAgo dateTime={createdAt} />}
      {score !== null && (
        <span>
          {score} {score === 1 ? "point" : "points"}
        </span>
      )}
      {commentCount !== null && (
        <span>
          {commentCount} {commentCount === 1 ? "comment" : "comments"}
        </span>
      )}
    </p>
  );
}
