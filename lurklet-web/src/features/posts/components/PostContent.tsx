import { getPostOptions } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { Markdown } from "@/components/Markdown";
import { PaneMenu } from "@/components/PaneMenu";
import { Separator } from "@/components/ui/separator";
import { PostError } from "@/features/posts/components/PostError";
import { PostLoading } from "@/features/posts/components/PostLoading";
import { PostMetadata } from "@/features/posts/components/PostMetadata";
import { useJsonQuery } from "@/hooks/useJsonQuery";
import { usePageTitle } from "@/hooks/usePageTitle";
import { PostAttachments } from "./PostAttachments";

export function PostContent({ postId }: { postId: string }) {
  const { query: post, jsonHref } = useJsonQuery(getPostOptions({ path: { id: postId } }));
  usePageTitle(
    post.isPending
      ? "Loading post"
      : post.isError
        ? post.error?.status === 404
          ? "Post not found"
          : "Unable to load post"
        : post.data.title,
  );

  const menu = <PaneMenu jsonHref={jsonHref} label="Post actions" />;

  if (!post.isSuccess) {
    return (
      <>
        {menu}
        {post.isPending ? <PostLoading /> : <PostError error={post.error} />}
      </>
    );
  }

  const value = post.data;

  return (
    <>
      {menu}
      <article className="flex flex-col gap-6 select-text">
        <div className="flex flex-col gap-2">
          <h1 className="min-w-0 text-2xl font-semibold wrap-anywhere">{value.title}</h1>
          <PostMetadata
            authorName={value.author?.name ?? null}
            createdAt={value.createdAt}
            score={value.score}
            commentCount={value.commentCount}
          />
        </div>
        <Separator />
        {value.markdown && (
          <section aria-label="Post content">
            <Markdown value={value.markdown} />
          </section>
        )}
        <PostAttachments attachments={value.attachments} />
      </article>
    </>
  );
}
