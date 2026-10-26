import { useParams } from "react-router";
import { Separator } from "@/components/ui/separator";
import { CommentDiscussion } from "@/features/comments/components/CommentDiscussion";
import { PostContent } from "@/features/posts/components/PostContent";

export function PostPage() {
  const { postId } = useParams();

  return (
    <div className="flex min-w-0 flex-col gap-6">
      <section aria-label="Post">
        <PostContent postId={postId!} />
      </section>
      <Separator />
      <CommentDiscussion />
    </div>
  );
}
