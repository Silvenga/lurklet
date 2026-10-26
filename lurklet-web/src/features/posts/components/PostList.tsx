import { Link } from "react-router";
import type { ListedPost } from "@/clients/lurklet-api/generated/types.gen";
import { Badge } from "@/components/ui/badge";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { PostMetadata } from "./PostMetadata";

type PostListProps = {
  subreddit: string;
  posts: ListedPost[];
};

export function PostList({ subreddit, posts }: PostListProps) {
  if (posts.length === 0) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyTitle>No posts</EmptyTitle>
          <EmptyDescription>No posts were returned for this listing.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <ul className="flex flex-col gap-6">
      {posts.map((post) => (
        <li key={post.id} className="flex flex-col gap-2 select-text">
          <div className="flex items-start gap-2">
            <h2 className="min-w-0 flex-1 text-lg font-medium">
              <Link
                className="hover:underline"
                to={`/r/${encodeURIComponent(subreddit)}/comments/${encodeURIComponent(post.id)}`}
              >
                {post.title ?? `Post ${post.id}`}
              </Link>
            </h2>
            {post.isPinned && <Badge variant="secondary">Pinned</Badge>}
          </div>
          <PostMetadata
            authorName={post.authorName}
            createdAt={post.createdAt}
            score={post.score}
            commentCount={post.commentCount}
          />
        </li>
      ))}
    </ul>
  );
}
