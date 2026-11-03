// @vitest-environment node

import { RouterContextProvider } from "react-router";
import { expect, test } from "vitest";
import type { ListSubredditPostsData } from "@/clients/lurklet-api/generated/types.gen";
import { postListLoader } from "./post-list-loader";

test.each([
  ["", { sort: "hot" }],
  ["?sort=hot&t=week&utm_source=example&after=", { sort: "hot" }],
  ["?sort=new&t=week", { sort: "new" }],
  ["?sort=top", { sort: "top", timeRange: "all" }],
  ["?sort=top&t=hour", { sort: "top", timeRange: "hour" }],
  ["?sort=top&t=day", { sort: "top", timeRange: "day" }],
  ["?sort=top&t=week", { sort: "top", timeRange: "week" }],
  ["?sort=top&t=month", { sort: "top", timeRange: "month" }],
  ["?sort=top&t=year", { sort: "top", timeRange: "year" }],
  ["?sort=top&t=all", { sort: "top", timeRange: "all" }],
  ["?sort=new&after=opaque%2B%2F%3D%26%20%3F%23", { sort: "new", after: "opaque+/=& ?#" }],
] satisfies Array<[string, NonNullable<ListSubredditPostsData["query"]>]>)(
  "When listing parameters are %s then the loader should select only the supported API inputs",
  (search, query: NonNullable<ListSubredditPostsData["query"]>) => {
    const request = new Request(`https://lurklet.test/r/Rust${search}`);

    const result = postListLoader({
      request,
      url: new URL(request.url),
      pattern: "/r/:subreddit",
      params: { subreddit: "Rust" },
      context: new RouterContextProvider(),
    });

    expect(result).toEqual({
      options: { path: { subreddit: "Rust" }, query },
      sort: query.sort,
      timeRange: query.timeRange,
    });
  },
);

test.each(["?sort=unsupported", "?sort=HOT", "?sort=top&t=unsupported"])(
  "When listing parameters are %s then the loader should reject the route",
  (search) => {
    const request = new Request(`https://lurklet.test/r/rust${search}`);

    const result = () =>
      postListLoader({
        request,
        url: new URL(request.url),
        pattern: "/r/:subreddit",
        params: { subreddit: "rust" },
        context: new RouterContextProvider(),
      });

    expect(result).toThrow(expect.objectContaining({ status: 404 }));
  },
);
