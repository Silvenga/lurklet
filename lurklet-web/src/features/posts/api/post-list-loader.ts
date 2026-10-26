import type { LoaderFunctionArgs } from "react-router";
import type { ListSubredditPostsData } from "@/clients/lurklet-api/generated/types.gen";

const TIME_RANGES = ["hour", "day", "week", "month", "year", "all"] as const;

export function postListLoader({ params, request }: LoaderFunctionArgs) {
  const url = new URL(request.url);
  const sort = url.searchParams.get("sort") ?? "hot";
  if (sort !== "hot" && sort !== "new" && sort !== "top") {
    throw new Response(null, { status: 404 });
  }

  const query: NonNullable<ListSubredditPostsData["query"]> = { sort };
  if (sort === "top") {
    const timeRange = TIME_RANGES.find((value) => value === (url.searchParams.get("t") ?? "all"));
    if (!timeRange) {
      throw new Response(null, { status: 404 });
    }
    query.timeRange = timeRange;
  }
  const after = url.searchParams.get("after");
  if (after) {
    query.after = after;
  }

  return {
    options: { path: { subreddit: params.subreddit! }, query },
    sort,
    timeRange: query.timeRange,
  } as const;
}
