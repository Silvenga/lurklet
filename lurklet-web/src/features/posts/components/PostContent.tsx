import { useQuery } from "@tanstack/react-query";
import { getPostOptions } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { Markdown } from "@/components/Markdown";
import { Separator } from "@/components/ui/separator";
import { PostError } from "@/features/posts/components/PostError";
import { PostLoading } from "@/features/posts/components/PostLoading";
import { PostMetadata } from "@/features/posts/components/PostMetadata";
import { PostAttachments } from "./PostAttachments";

export function PostContent({ postId }: { postId: string }) {
  const post = useQuery(getPostOptions({ path: { id: postId } }));

  if (post.isPending) {
    return <PostLoading />;
  }
  if (post.isError) {
    return <PostError error={post.error} />;
  }

  const value = post.data;

  return (
    <article className="flex flex-col gap-6 select-text">
      <div className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold">{value.title}</h1>
        <PostMetadata
          authorName={value.author?.name ?? null}
          createdAt={value.createdAt}
          score={value.score}
          commentCount={value.commentCount}
        />
      </div>
      <Separator />
      {value.bodyMarkdown && (
        <section aria-label="Post content">
          <Markdown>{value.bodyMarkdown}</Markdown>
        </section>
      )}
      <PostAttachments attachments={value.attachments} />
    </article>
  );
}
