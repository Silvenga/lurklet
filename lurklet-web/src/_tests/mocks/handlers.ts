import { http, HttpResponse } from "msw/http";
import type { Problem } from "@/clients/lurklet-api/generated/types.gen";
import {
  COMMENTS_RESPONSE,
  COMMENT_THREAD_RESPONSE,
  HEALTH_RESPONSE,
  POST_RESPONSE,
  SUBREDDIT_NEXT_PAGE_RESPONSE,
  SUBREDDIT_PAGE_RESPONSE,
  SUBREDDIT_RESPONSE,
} from "./fixtures";

export const handlers = [
  http.get("*/api/posts/:id/comments", ({ params, request }) =>
    HttpResponse.json(
      params.id === POST_RESPONSE.id && !new URL(request.url).searchParams.has("cursor")
        ? {
            ...COMMENTS_RESPONSE,
            appliedSort: new URL(request.url).searchParams.get("sort") ?? "best",
          }
        : { appliedSort: new URL(request.url).searchParams.get("sort") ?? "best", items: [] },
      { headers: { "Cache-Control": "no-store" } },
    ),
  ),
  http.get("*/api/comments/:id/thread", ({ params }) =>
    params.id === COMMENTS_RESPONSE.items[0]!.comment.id
      ? HttpResponse.json(COMMENT_THREAD_RESPONSE, { headers: { "Cache-Control": "no-store" } })
      : HttpResponse.json(
          { type: "about:blank", title: "Not Found", status: 404 } satisfies Problem,
          { status: 404 },
        ),
  ),
  http.get("*/api/health", () =>
    HttpResponse.json(HEALTH_RESPONSE, { headers: { "Cache-Control": "no-store" } }),
  ),
  http.get("*/api/subreddits/:subreddit", ({ params }) =>
    HttpResponse.json(
      params.subreddit === "rust"
        ? SUBREDDIT_RESPONSE
        : {
            ...SUBREDDIT_RESPONSE,
            id: `mock-${params.subreddit}`,
            name: String(params.subreddit),
            title: `About r/${params.subreddit}`,
            shortDescription: null,
            descriptionMarkdown: null,
            styles: null,
          },
      { headers: { "Cache-Control": "no-store" } },
    ),
  ),
  http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
    const query = new URL(request.url).searchParams;
    const after = query.get("after");
    const page = after
      ? after === SUBREDDIT_PAGE_RESPONSE.nextCursor
        ? SUBREDDIT_NEXT_PAGE_RESPONSE
        : { posts: [], nextCursor: null }
      : {
          ...SUBREDDIT_PAGE_RESPONSE,
          posts:
            query.get("sort") === "new"
              ? SUBREDDIT_PAGE_RESPONSE.posts.toSorted((a, b) =>
                  b.createdAt.localeCompare(a.createdAt),
                )
              : query.get("sort") === "top"
                ? SUBREDDIT_PAGE_RESPONSE.posts.toSorted((a, b) => b.score - a.score)
                : SUBREDDIT_PAGE_RESPONSE.posts,
        };
    return HttpResponse.json(page, { headers: { "Cache-Control": "no-store" } });
  }),
  http.get("*/api/posts/:id", ({ params }) => {
    if (params.id !== POST_RESPONSE.id) {
      return HttpResponse.json(
        { type: "about:blank", title: "Not Found", status: 404 } satisfies Problem,
        {
          status: 404,
          headers: { "Content-Type": "application/problem+json", "Cache-Control": "no-store" },
        },
      );
    }
    return HttpResponse.json(POST_RESPONSE, { headers: { "Cache-Control": "no-store" } });
  }),
];
