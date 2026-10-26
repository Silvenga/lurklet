import type { ListedPost } from "@/clients/lurklet-api/generated/types.gen";

type PostMetadataProps = Pick<ListedPost, "authorName" | "createdAt" | "score" | "commentCount">;

const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

export function PostMetadata({ authorName, createdAt, score, commentCount }: PostMetadataProps) {
  return (
    <p className="text-muted-foreground flex flex-wrap gap-x-3 gap-y-1 text-sm">
      {authorName !== null && <span>u/{authorName}</span>}
      {createdAt !== null && (
        <time dateTime={createdAt}>{DATE_FORMAT.format(new Date(createdAt))}</time>
      )}
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
