import { Link, useParams } from "react-router";
import { apiQueryOptions } from "@/clients/lurklet-api/query-options";
import { Markdown } from "@/components/Markdown";
import { PaneMenu } from "@/components/PaneMenu";
import { Separator } from "@/components/ui/separator";
import { PostError } from "@/features/posts/components/PostError";
import { PostLoading } from "@/features/posts/components/PostLoading";
import { PostMetadata } from "@/features/posts/components/PostMetadata";
import { useJsonQuery } from "@/hooks/useJsonQuery";
import { usePageTitle } from "@/hooks/usePageTitle";
import { CrosspostPreview } from "./CrosspostPreview";
import { PostAttachments } from "./PostAttachments";
import { PostFlair } from "./PostFlair";

export function PostContent({ postId }: { postId: string }) {
  const { subreddit } = useParams();
  const {
    query: post,
    jsonHref,
    fetchedAt,
  } = useJsonQuery(apiQueryOptions.getPost({ path: { id: postId } }));
  usePageTitle(
    post.isPending
      ? "Loading post"
      : post.isError
        ? post.error?.status === 404
          ? "Post not found"
          : "Unable to load post"
        : post.data.title,
  );

  const permalink = post.data?.permalink
    ? URL.parse(post.data.permalink, "https://reddit.com")
    : null;
  const redditHref =
    permalink?.protocol === "https:" &&
    (permalink.hostname === "reddit.com" || permalink.hostname.endsWith(".reddit.com"))
      ? permalink.href
      : `https://www.reddit.com/r/${encodeURIComponent(subreddit!)}/comments/${encodeURIComponent(postId)}/`;

  const header = (
    <>
      <PaneMenu
        jsonHref={jsonHref}
        fetchedAt={fetchedAt}
        redditHref={redditHref}
        label="Post actions"
      />
      <div className="flex min-w-0 flex-wrap items-center gap-2 pr-10">
        <Link
          className="text-muted-foreground min-w-0 text-sm wrap-anywhere hover:underline"
          to={`/r/${encodeURIComponent(subreddit!)}`}
        >
          r/{subreddit}
        </Link>
        <PostFlair flair={post.isSuccess ? post.data.flair : null} />
      </div>
    </>
  );

  if (!post.isSuccess) {
    return (
      <>
        {header}
        {post.isPending ? <PostLoading /> : <PostError error={post.error} />}
      </>
    );
  }

  const value = post.data;

  return (
    <>
      {header}
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
            <Markdown className="prose-sm max-w-none" value={value.markdown} />
          </section>
        )}
        <PostAttachments attachments={value.attachments} />
        {value.crosspostId && (
          <CrosspostPreview
            key={`${value.id}:${value.crosspostId}`}
            postId={value.crosspostId}
            isSensitive={value.isNsfw || value.isSpoiler}
          />
        )}
      </article>
    </>
  );
}
