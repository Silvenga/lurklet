import { http, HttpResponse } from "msw/http";
import type { CommentThread, Problem } from "@/clients/lurklet-api/generated/types.gen";
import {
  COMMENTS_RESPONSE,
  COMMENT_THREAD_RESPONSE,
  CROSSPOST_COMMENTS_RESPONSE,
  CROSSPOST_RESPONSE,
  HEALTH_RESPONSE,
  MEDIA_PNG_BASE64,
  ORIGINAL_POST_RESPONSE,
  POST_RESPONSE,
  SUBREDDIT_NEXT_PAGE_RESPONSE,
  SUBREDDIT_PAGE_RESPONSE,
  SUBREDDIT_RESPONSE,
  VIDEO_POST_RESPONSE,
  VIDEO_SOURCE_URL,
} from "./fixtures";
import videoData from "./video.mp4?inline";

const VIDEO_BYTES = Uint8Array.from(atob(videoData.split(",")[1]!), (char) => char.charCodeAt(0));

export const handlers = [
  http.all("*/api/media/:origin/*", ({ request }) => {
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new HttpResponse(null, { status: 405, headers: { Allow: "GET, HEAD" } });
    }
    const proxy = new URL(request.url);
    const url = `https://${proxy.pathname.slice("/api/media/".length)}${proxy.search}`;
    if (url !== VIDEO_SOURCE_URL) {
      return HttpResponse.arrayBuffer(
        request.method === "HEAD"
          ? undefined
          : Uint8Array.from(atob(MEDIA_PNG_BASE64), (char) => char.charCodeAt(0)).buffer,
        { headers: { "Content-Type": "image/png", "Cache-Control": "no-store" } },
      );
    }
    const headers = new Headers({
      "Content-Type": "video/mp4",
      "Cache-Control": "public, max-age=3600",
      "Accept-Ranges": "bytes",
      ETag: '"fixture-video"',
    });
    let start = 0;
    let end = VIDEO_BYTES.length - 1;
    let status = 200;
    const range = /^bytes=(\d*)-(\d*)$/.exec(request.headers.get("Range") ?? "");
    const validator = request.headers.get("If-Range");
    if (
      request.method === "GET" &&
      range &&
      (range[1] || range[2]) &&
      (!validator || validator === headers.get("ETag"))
    ) {
      const first = range[1] ? Number(range[1]) : null;
      const last = range[2] ? Number(range[2]) : null;
      if (
        (first === null || Number.isSafeInteger(first)) &&
        (last === null || Number.isSafeInteger(last)) &&
        (first === null || last === null || first <= last)
      ) {
        start = first ?? Math.max(0, VIDEO_BYTES.length - last!);
        end = first === null || last === null ? end : Math.min(end, last);
        if (start > end) {
          return new HttpResponse(null, {
            status: 416,
            headers: {
              "Content-Range": `bytes */${VIDEO_BYTES.length}`,
              "Cache-Control": "no-store",
            },
          });
        }
        status = 206;
        headers.set("Content-Range", `bytes ${start}-${end}/${VIDEO_BYTES.length}`);
      }
    }
    headers.set("Content-Length", String(end - start + 1));
    return HttpResponse.arrayBuffer(
      request.method === "HEAD" ? undefined : VIDEO_BYTES.slice(start, end + 1).buffer,
      { status, headers },
    );
  }),
  http.get("*/api/posts/:id/comments", ({ params, request }) => {
    const comments =
      params.id === POST_RESPONSE.id
        ? COMMENTS_RESPONSE
        : params.id === CROSSPOST_RESPONSE.id
          ? CROSSPOST_COMMENTS_RESPONSE
          : null;
    return HttpResponse.json(
      comments && !new URL(request.url).searchParams.has("cursor")
        ? {
            ...comments,
            appliedSort: new URL(request.url).searchParams.get("sort") ?? "best",
          }
        : { appliedSort: new URL(request.url).searchParams.get("sort") ?? "best", items: [] },
      { headers: { "Cache-Control": "no-store", Age: "7200" } },
    );
  }),
  http.get("*/api/comments/:id/thread", ({ params }) => {
    const thread =
      params.id === COMMENTS_RESPONSE.items[0]!.comment.id
        ? COMMENT_THREAD_RESPONSE
        : params.id === CROSSPOST_COMMENTS_RESPONSE.items[0]!.comment.id
          ? ({
              postId: CROSSPOST_RESPONSE.id,
              forest: CROSSPOST_COMMENTS_RESPONSE,
            } satisfies CommentThread)
          : null;
    return thread
      ? HttpResponse.json(thread, { headers: { "Cache-Control": "no-store", Age: "86400" } })
      : HttpResponse.json(
          { type: "about:blank", title: "Not Found", status: 404 } satisfies Problem,
          { status: 404 },
        );
  }),
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
      { headers: { "Cache-Control": "no-store", Age: "0" } },
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
    return HttpResponse.json(page, { headers: { "Cache-Control": "no-store", Age: "600" } });
  }),
  http.get("*/api/posts/:id", ({ params }) => {
    const post = [
      POST_RESPONSE,
      CROSSPOST_RESPONSE,
      ORIGINAL_POST_RESPONSE,
      VIDEO_POST_RESPONSE,
    ].find((value) => value.id === params.id);
    if (!post) {
      return HttpResponse.json(
        { type: "about:blank", title: "Not Found", status: 404 } satisfies Problem,
        {
          status: 404,
          headers: { "Content-Type": "application/problem+json", "Cache-Control": "no-store" },
        },
      );
    }
    return HttpResponse.json(post, {
      headers: { "Cache-Control": "no-store", Age: "3060" },
    });
  }),
];
