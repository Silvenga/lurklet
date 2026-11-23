import { Link } from "react-router";
import type { ListedPost } from "@/clients/lurklet-api/generated/types.gen";
import { EmptyResult } from "@/components/EmptyResult";
import { Badge } from "@/components/ui/badge";
import { PostFlair } from "./PostFlair";
import { PostMetadata } from "./PostMetadata";
import { PostThumbnail } from "./PostThumbnail";
import { SpoilerBadge } from "./SpoilerBadge";

type PostListProps = {
  subreddit: string;
  posts: ListedPost[];
};

export function PostList({ subreddit, posts }: PostListProps) {
  if (posts.length === 0) {
    return <EmptyResult>No posts in this listing.</EmptyResult>;
  }

  return (
    <ul className="flex flex-col gap-6">
      {posts.map((post) => (
        <li key={post.id} className="flex items-start gap-3 select-text">
          {post.thumbnail && (
            <PostThumbnail
              key={post.thumbnail.url}
              thumbnail={post.thumbnail}
              title={post.title ?? `Post ${post.id}`}
              isSensitive={post.isNsfw || post.isSpoiler || post.thumbnail.isObfuscated === true}
            />
          )}
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <div className="flex flex-wrap items-start gap-2">
              <h2 className="min-w-0 flex-1 basis-1/2 text-lg font-medium wrap-anywhere">
                <Link
                  className="hover:underline"
                  to={`/r/${encodeURIComponent(subreddit)}/comments/${encodeURIComponent(post.id)}`}
                >
                  {post.title ?? `Post ${post.id}`}
                </Link>
              </h2>
              {(post.flair || post.isPinned || post.isSpoiler) && (
                <div className="ms-auto flex max-w-full min-w-0 flex-wrap items-start gap-2">
                  <PostFlair flair={post.flair} />
                  {post.isSpoiler && <SpoilerBadge />}
                  {post.isPinned && (
                    <Badge variant="secondary" className="font-normal">
                      Pinned
                    </Badge>
                  )}
                </div>
              )}
            </div>
            <PostMetadata
              authorName={post.authorName}
              createdAt={post.createdAt}
              score={post.score}
              commentCount={post.commentCount}
              crosspostId={post.crosspostId}
            />
          </div>
        </li>
      ))}
    </ul>
  );
}
