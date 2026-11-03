import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router";
import { apiQueryOptions } from "@/clients/lurklet-api/query-options";
import { Markdown } from "@/components/Markdown";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { PostAttachments } from "./PostAttachments";
import { PostMetadata } from "./PostMetadata";

export function CrosspostPreview({
  postId,
  isSensitive,
}: {
  postId: string;
  isSensitive: boolean;
}) {
  const post = useQuery(apiQueryOptions.getPost({ path: { id: postId } }));
  const [isRevealed, setIsRevealed] = useState(false);

  if (!post.isSuccess) {
    const isNotFound = post.error?.status === 404;

    return (
      <section aria-label="Original post" className="min-w-0 select-none">
        {post.isPending ? (
          <div role="status" className="flex flex-col gap-3">
            <span className="sr-only">Loading original post</span>
            <Skeleton className="h-5 w-3/4" />
            <Skeleton className="h-16 w-full" />
          </div>
        ) : (
          <Alert variant={isNotFound ? "default" : "destructive"}>
            <AlertTitle>
              {isNotFound ? "Original post unavailable" : "Unable to load original post"}
            </AlertTitle>
            <AlertDescription>
              {isNotFound ? "The original post could not be found." : "Please try again."}
            </AlertDescription>
            {!isNotFound && (
              <Button
                variant="outline"
                size="sm"
                className="mt-2 w-fit"
                onPress={() => {
                  void post.refetch();
                }}
              >
                Retry
              </Button>
            )}
          </Alert>
        )}
      </section>
    );
  }

  const value = post.data;
  const subreddit = value.community?.name;
  const isHidden = (isSensitive || value.isNsfw || value.isSpoiler) && !isRevealed;

  return (
    <section aria-label="Original post" className="min-w-0 select-none">
      <Card>
        <CardHeader>
          <CardDescription className="wrap-anywhere">
            {subreddit ? (
              <Link className="hover:underline" to={`/r/${encodeURIComponent(subreddit)}`}>
                r/{subreddit}
              </Link>
            ) : (
              "Unknown community"
            )}
          </CardDescription>
          <CardTitle>
            <h2 className="min-w-0 wrap-anywhere">
              {subreddit ? (
                <Link
                  className="hover:underline"
                  to={`/r/${encodeURIComponent(subreddit)}/comments/${encodeURIComponent(value.id)}`}
                >
                  {value.title}
                </Link>
              ) : (
                value.title
              )}
            </h2>
          </CardTitle>
          <PostMetadata
            authorName={value.author?.name ?? null}
            createdAt={value.createdAt}
            score={value.score}
            commentCount={value.commentCount}
          />
        </CardHeader>
        {(value.markdown !== null || value.attachments.length > 0) && (
          <CardContent className="flex min-w-0 flex-col gap-4 select-text">
            {isHidden ? (
              <>
                <p>NSFW or spoiler content is hidden.</p>
                <Button variant="outline" className="w-fit" onPress={() => setIsRevealed(true)}>
                  Reveal original content
                </Button>
              </>
            ) : (
              <>
                {value.markdown && (
                  <Markdown className="prose-sm max-w-none" value={value.markdown} />
                )}
                <PostAttachments attachments={value.attachments} />
              </>
            )}
          </CardContent>
        )}
      </Card>
    </section>
  );
}
