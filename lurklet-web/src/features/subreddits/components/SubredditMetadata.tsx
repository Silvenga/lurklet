import { useQuery } from "@tanstack/react-query";
import { getSubredditOptions } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { Markdown } from "@/components/Markdown";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";

const NUMBER_FORMAT = new Intl.NumberFormat();
const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
const MARKDOWN_CLASS = "prose-sm max-w-none overflow-x-auto wrap-anywhere";

export function SubredditMetadata({ subreddit }: { subreddit: string }) {
  const metadata = useQuery(getSubredditOptions({ path: { subreddit } }));

  if (metadata.isPending) {
    return (
      <div role="status" className="flex flex-col gap-4">
        <span className="sr-only">Loading subreddit details</span>
        <Skeleton className="h-6 w-3/4" />
        <Skeleton className="h-24 w-full" />
      </div>
    );
  }

  if (metadata.isError) {
    const error: unknown = metadata.error;
    const isNotFound =
      typeof error === "object" && error !== null && "status" in error && error.status === 404;
    return (
      <Alert variant={isNotFound ? "default" : "destructive"}>
        <AlertTitle>
          {isNotFound ? "Subreddit details not found" : "Unable to load subreddit details"}
        </AlertTitle>
        <AlertDescription>
          {isNotFound ? "No metadata was found for this subreddit." : "Please try again later."}
        </AlertDescription>
      </Alert>
    );
  }

  const value = metadata.data;
  const counts =
    value.kind === "available"
      ? ([
          ["Weekly active users", value.weeklyActiveUserCount],
          ["Weekly contributions", value.weeklyContributionCount],
        ] as const)
      : [];

  return (
    <div className="flex min-w-0 flex-col gap-4 wrap-anywhere select-text">
      {value.kind === "available" ? (
        <>
          {value.title && <p className="font-medium">{value.title}</p>}
          {value.shortDescription && (
            <p className="prose prose-sm dark:prose-invert max-w-none">{value.shortDescription}</p>
          )}
          {(value.isNsfw || value.isQuarantined) && (
            <div className="flex flex-wrap gap-2">
              {value.isNsfw && <Badge variant="outline">NSFW</Badge>}
              {value.isQuarantined && <Badge variant="outline">Quarantined</Badge>}
            </div>
          )}
          {(value.subscriberCount !== null ||
            value.createdAt ||
            counts.some(([, count]) => count !== null)) && (
            <dl className="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
              {value.subscriberCount !== null && (
                <div>
                  <dt className="text-muted-foreground whitespace-nowrap">Subscribers</dt>
                  <dd>{NUMBER_FORMAT.format(value.subscriberCount)}</dd>
                </div>
              )}
              {value.createdAt && (
                <div>
                  <dt className="text-muted-foreground whitespace-nowrap">Created</dt>
                  <dd>
                    <time dateTime={value.createdAt}>
                      {DATE_FORMAT.format(new Date(value.createdAt))}
                    </time>
                  </dd>
                </div>
              )}
              {counts.map(
                ([label, count]) =>
                  count !== null && (
                    <div key={label} className="col-span-2">
                      <dt className="text-muted-foreground whitespace-nowrap">{label}</dt>
                      <dd>{NUMBER_FORMAT.format(count)}</dd>
                    </div>
                  ),
              )}
            </dl>
          )}
        </>
      ) : (
        <Alert>
          <AlertTitle>{value.banTitle || "Subreddit unavailable"}</AlertTitle>
          <AlertDescription>
            {value.forbiddenReason && <p>{value.forbiddenReason}</p>}
            {value.banMessageMarkdown && (
              <Markdown className={MARKDOWN_CLASS}>{value.banMessageMarkdown}</Markdown>
            )}
            {!value.forbiddenReason && !value.banMessageMarkdown && (
              <p>This subreddit is unavailable.</p>
            )}
          </AlertDescription>
        </Alert>
      )}
      {[
        ["Community notice", value.interstitialWarningMarkdown],
        ["Quarantine notice", value.quarantineMessageMarkdown],
      ].map(
        ([title, body]) =>
          body && (
            <Alert key={title}>
              <AlertTitle>{title}</AlertTitle>
              <AlertDescription>
                <Markdown className={MARKDOWN_CLASS}>{body}</Markdown>
              </AlertDescription>
            </Alert>
          ),
      )}
    </div>
  );
}
