import { Link, useParams } from "react-router";
import { Separator } from "@/components/ui/separator";
import { CommentDiscussion } from "@/features/comments/components/CommentDiscussion";
import { PostContent } from "@/features/posts/components/PostContent";

export function PostPage() {
  const { subreddit, postId } = useParams();

  return (
    <div className="flex min-w-0 flex-col gap-6">
      <section aria-label="Post" className="flex min-w-0 flex-col gap-2">
        <Link
          className="text-muted-foreground w-fit text-sm hover:underline"
          to={`/r/${encodeURIComponent(subreddit!)}`}
        >
          r/{subreddit}
        </Link>
        <PostContent postId={postId!} />
      </section>
      <Separator />
      <CommentDiscussion />
    </div>
  );
}
