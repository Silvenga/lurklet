import { QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw/http";
import { createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test } from "vitest";
import {
  POST_RESPONSE,
  SUBREDDIT_NEXT_PAGE_RESPONSE,
  SUBREDDIT_PAGE_RESPONSE,
  SUBREDDIT_RESPONSE,
} from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import {
  getPostQueryKey,
  listSubredditPostsQueryKey,
} from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import { createQueryClient } from "./query-client";
import { APP_ROUTES } from "./router";

const POST_PATH = `/r/rust/comments/${POST_RESPONSE.id}`;

const CONTENT_REQUESTS = [
  ["listing", "/r/rust", "*/api/subreddits/:subreddit/posts", SUBREDDIT_PAGE_RESPONSE],
  ["post", POST_PATH, "*/api/posts/:id", POST_RESPONSE],
] as const;

function renderPath(path: string, queryClient = createQueryClient()) {
  const router = createMemoryRouter(APP_ROUTES, { initialEntries: [path] });
  const { unmount } = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  onTestFinished(() => {
    unmount();
    router.dispose();
    queryClient.clear();
  });
  return router;
}

test("When a subreddit is entered then its listing should link to the supplied post", async () => {
  const user = userEvent.setup();
  renderPath("/");

  await user.type(screen.getByRole("textbox", { name: "r/" }), "rust{Enter}");

  const post = await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(
    within(screen.getByRole("region", { name: "Page content" })).getAllByRole("listitem"),
  ).toHaveLength(SUBREDDIT_PAGE_RESPONSE.posts.length);
  expect(post).toHaveAttribute("href", POST_PATH);

  await user.click(post);

  expect(await screen.findByRole("heading", { level: 1, name: POST_RESPONSE.title })).toBeVisible();
  const article = screen.getByRole("article");
  expect(within(article).getByText("u/seino_chan")).toBeVisible();
  expect(within(article).getByText("44 points")).toBeVisible();
  expect(
    within(article).getByRole("link", { name: POST_RESPONSE.attachments[0]!.url }),
  ).toHaveAttribute("href", POST_RESPONSE.attachments[0]!.url);
});

test("When a post has a link attachment then it should open an untrusted link without referrer or opener access", async () => {
  renderPath(POST_PATH);

  const article = await screen.findByRole("article");
  const link = within(article).getByRole("link", { name: POST_RESPONSE.attachments[0]!.url });

  expect(link).toHaveAttribute("href", POST_RESPONSE.attachments[0]!.url);
  expect(link).toHaveAttribute("target", "_blank");
  expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow ugc external");
  expect(screen.queryByRole("region", { name: "Post content" })).not.toBeInTheDocument();
  const separator = within(article).getByRole("separator");
  expect(separator.tagName).toBe("HR");
  expect(separator.previousElementSibling).toContainElement(
    within(article).getByRole("heading", { name: POST_RESPONSE.title }),
  );
  expect(separator.nextElementSibling).toBe(link);
});

test("When a post contains unsupported attachments then their placeholders should appear outside its Markdown body", async () => {
  server.use(
    http.get("*/api/posts/:id", () =>
      HttpResponse.json({
        ...POST_RESPONSE,
        bodyMarkdown: "Readable post text.",
        attachments: [
          {
            kind: "image",
            url: "https://images.example.com/diagram.png",
            altText: "A diagram",
            caption: "Diagram caption",
          },
          { kind: "video", posterUrl: null, sources: [] },
        ],
      }),
    ),
  );

  renderPath(POST_PATH);

  const article = await screen.findByRole("article");
  const body = within(article).getByRole("region", { name: "Post content" });
  const image = within(article).getByRole("img", { name: "Image unavailable: A diagram" });
  const video = within(article).getByRole("img", { name: "Video unavailable" });
  expect(body).toHaveTextContent("Readable post text.");
  expect(body).not.toContainElement(image);
  expect(body).not.toContainElement(video);
  expect(body.nextElementSibling).toBe(image.closest("figure"));
  expect(image.closest("figure")?.nextElementSibling).toBe(video);
  expect(
    article.querySelector(
      "img, video, audio, iframe, source, object, embed, [src], [srcset], [poster]",
    ),
  ).toBeNull();
});

test("When a listing is opened then it should not request individual post details", async () => {
  let listingRequests = 0;
  let postRequests = 0;
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", () => {
      listingRequests++;
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
    http.get("*/api/posts/:id", () => {
      postRequests++;
      return HttpResponse.json(POST_RESPONSE);
    }),
  );

  renderPath("/r/rust");

  expect(await screen.findByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(listingRequests).toBe(1);
  expect(postRequests).toBe(0);
});

test("When a Markdown Reddit link is followed then it should navigate locally and preserve the shared layout", async () => {
  const user = userEvent.setup();
  server.use(
    http.get("*/api/posts/:id", () =>
      HttpResponse.json({
        ...POST_RESPONSE,
        bodyMarkdown: "[Rust listing](https://old.reddit.com/r/rust/top?t=week#results)",
      }),
    ),
  );
  const router = renderPath(POST_PATH);
  const link = await screen.findByRole("link", { name: "Rust listing" });
  const header = screen.getByRole("banner");
  const sidebar = screen.getByRole("complementary");

  await user.click(link);

  expect(await screen.findByRole("heading", { level: 1, name: "r/rust" })).toBeVisible();
  expect(router.state.location.pathname).toBe("/r/rust");
  expect(new URLSearchParams(router.state.location.search).get("sort")).toBe("top");
  expect(new URLSearchParams(router.state.location.search).get("t")).toBe("week");
  expect(router.state.location.hash).toBe("#results");
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("complementary")).toBe(sidebar);
});

test("When Markdown links to an unsupported Reddit path then it should show the local not-found page", async () => {
  const user = userEvent.setup();
  server.use(
    http.get("*/api/posts/:id", () =>
      HttpResponse.json({
        ...POST_RESPONSE,
        bodyMarkdown: "[Rules](https://www.reddit.com/r/rust/wiki/rules)",
      }),
    ),
  );
  const router = renderPath(POST_PATH);
  const link = await screen.findByRole("link", { name: "Rules" });

  await user.click(link);

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeVisible();
  expect(router.state.location.pathname).toBe("/r/rust/wiki/rules");
});

test.each([
  ["/r/rust", "hot"],
  ["/r/rust/", "hot"],
  ["/r/rust/hot", "hot"],
  ["/r/rust/new/", "new"],
  ["/r/rust/top?t=week", "top"],
  ["/r/rust?sort=hot", "hot"],
  ["/r/rust?sort=new", "new"],
  ["/r/rust?sort=top&t=week", "top"],
])("When %s is opened then it should request the %s listing", async (path, sort) => {
  const requests: URL[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      requests.push(new URL(request.url));
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );

  renderPath(path);

  expect(await screen.findByRole("heading", { level: 1, name: "r/rust" })).toBeVisible();
  expect(await screen.findByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(requests).toHaveLength(1);
  expect(requests[0]?.pathname).toBe("/api/subreddits/rust/posts");
  expect(requests[0]?.searchParams.get("sort")).toBe(sort);
  expect(requests[0]?.searchParams.get("timeRange")).toBe(sort === "top" ? "week" : null);
  expect(
    within(screen.getByRole("complementary")).getByRole("link", { name: "r/rust" }),
  ).toHaveAttribute("href", "/r/rust");
  expect(
    await within(screen.getByRole("complementary")).findByText(SUBREDDIT_RESPONSE.title, {
      selector: "p",
    }),
  ).toBeVisible();
});

test.each([
  "/missing",
  "/r/rust/unsupported",
  "/r/rust/comments",
  "/r/rust/s/share-token",
  "/abc123",
  "/comments/abc123",
  `${POST_PATH}/title/comment456/unsupported`,
  "/r/rust/top?t=unsupported",
  "/r/rust?sort=unsupported",
  "/r/rust?sort=top&t=unsupported",
])(
  "When %s is unsupported then it should show not found without replacing the header",
  async (path) => {
    renderPath(path);

    expect(await screen.findByRole("heading", { name: "Page not found" })).toBeVisible();
    expect(within(screen.getByRole("banner")).getByRole("link", { name: "Lurklet" })).toBeVisible();
  },
);

test.each([
  POST_PATH,
  `${POST_PATH}/`,
  `${POST_PATH}/an_outdated_title`,
  `${POST_PATH}/an_outdated_title/`,
])(
  "When %s is opened then it should request the post by ID and render its Markdown",
  async (path) => {
    const requests: string[] = [];
    server.use(
      http.get("*/api/posts/:id", ({ request }) => {
        requests.push(new URL(request.url).pathname);
        return HttpResponse.json({ ...POST_RESPONSE, bodyMarkdown: "A **Markdown** post." });
      }),
    );

    renderPath(path);

    expect(
      await screen.findByRole("heading", { level: 1, name: POST_RESPONSE.title }),
    ).toBeVisible();
    expect(requests).toEqual([`/api/posts/${POST_RESPONSE.id}`]);
    expect(screen.getByText("Markdown").tagName).toBe("STRONG");
    expect(screen.getByRole("article")).toHaveClass("select-text");
    const content = screen.getByRole("region", { name: "Post content" });
    expect(content.tagName).toBe("SECTION");
    expect(content).not.toHaveClass("border");
    expect(content).not.toHaveClass("p-4");
    const separator = within(screen.getByRole("article")).getByRole("separator");
    expect(separator.tagName).toBe("HR");
    expect(separator.nextElementSibling).toBe(content);
    expect(within(content).getByText("Markdown")).toBeVisible();
    expect(
      within(content).queryByRole("heading", { name: POST_RESPONSE.title }),
    ).not.toBeInTheDocument();
    expect(within(content).queryByText("u/seino_chan")).not.toBeInTheDocument();
    const attachment = screen.getByRole("link", { name: POST_RESPONSE.attachments[0]!.url });
    expect(content).not.toContainElement(attachment);
    expect(attachment).toBeVisible();
    expect(screen.getAllByRole("main")).toHaveLength(1);
  },
);

test("When a post is loading then only the post region should show a loading state", async () => {
  const pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/posts/:id", () => pending.promise));

  renderPath(POST_PATH);

  expect(
    await within(await screen.findByRole("region", { name: "Page content" })).findByRole("status"),
  ).toHaveTextContent("Loading post");
  const header = screen.getByRole("banner");
  const sidebar = screen.getByRole("complementary");
  expect(within(header).getByRole("link", { name: "Lurklet" })).toBeVisible();
  expect(within(sidebar).getByRole("heading", { name: "r/rust" })).toBeVisible();

  await act(async () => pending.resolve(HttpResponse.json(POST_RESPONSE)));

  expect(await screen.findByRole("heading", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Page content" })).queryByRole("status"),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("complementary")).toBe(sidebar);
});

test("When only the slug or query changes then the post should not be requested again", async () => {
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id", () => {
      requests++;
      return HttpResponse.json(POST_RESPONSE);
    }),
  );
  const router = renderPath(`${POST_PATH}/old_title`);
  await screen.findByRole("heading", { name: POST_RESPONSE.title });
  const sidebar = screen.getByRole("complementary");

  await act(async () => {
    await router.navigate(`${POST_PATH}/new_title/?utm_source=example`);
  });

  expect(requests).toBe(1);
  expect(screen.getByRole("heading", { name: POST_RESPONSE.title })).toBeVisible();
  expect(screen.getByRole("complementary")).toBe(sidebar);
});

test("When its query is explicitly invalidated then the post should be fetched again", async () => {
  let requests = 0;
  server.use(
    http.get("*/api/posts/:id", () => {
      requests++;
      return HttpResponse.json({
        ...POST_RESPONSE,
        title: requests === 1 ? POST_RESPONSE.title : "Updated post",
      });
    }),
  );
  const queryClient = createQueryClient();
  renderPath(POST_PATH, queryClient);
  await screen.findByRole("heading", { name: POST_RESPONSE.title });

  await act(async () => {
    await queryClient.invalidateQueries({
      queryKey: getPostQueryKey({ path: { id: POST_RESPONSE.id } }),
    });
  });

  expect(await screen.findByRole("heading", { name: "Updated post" })).toBeVisible();
  expect(requests).toBe(2);
});

test("When navigating between posts then the new ID should be fetched without replacing the subreddit layout", async () => {
  const requests: string[] = [];
  server.use(
    http.get("*/api/posts/:id", ({ params, request }) => {
      requests.push(new URL(request.url).pathname);
      return HttpResponse.json({
        ...POST_RESPONSE,
        id: String(params.id),
        title: `Post ${params.id}`,
      });
    }),
  );
  const router = renderPath(POST_PATH);
  await screen.findByRole("heading", { name: `Post ${POST_RESPONSE.id}` });
  const header = screen.getByRole("banner");
  const sidebar = screen.getByRole("complementary");

  await act(async () => {
    await router.navigate("/r/rust/comments/def456/another_post");
  });

  expect(await screen.findByRole("heading", { name: "Post def456" })).toBeVisible();
  expect(requests).toEqual([`/api/posts/${POST_RESPONSE.id}`, "/api/posts/def456"]);
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("complementary")).toBe(sidebar);
});

test("When navigating between listings and posts then the subreddit layout should stay mounted", async () => {
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  const header = screen.getByRole("banner");
  const sidebar = screen.getByRole("complementary");

  await act(async () => {
    await router.navigate("/r/rust?sort=top&t=week");
  });

  expect(screen.getByRole("heading", { level: 1, name: "r/rust" })).toBeVisible();
  expect(router.state.location.search).toBe("?sort=top&t=week");
  expect(screen.getByRole("complementary")).toBe(sidebar);

  await act(async () => {
    await router.navigate(`${POST_PATH}/title`);
  });

  expect(await screen.findByRole("heading", { name: POST_RESPONSE.title })).toBeVisible();
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("complementary")).toBe(sidebar);

  await act(async () => {
    await router.navigate(-1);
  });

  expect(screen.getByRole("heading", { level: 1, name: "r/rust" })).toBeVisible();
  expect(router.state.location.search).toBe("?sort=top&t=week");
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("complementary")).toBe(sidebar);
});

test("When navigating to another subreddit then the sidebar should reflect the new route", async () => {
  server.use(
    http.get("*/api/posts/def456", () =>
      HttpResponse.json({
        ...POST_RESPONSE,
        id: "def456",
        title: "A TypeScript post",
        community: { id: "community2", name: "typescript" },
      }),
    ),
  );
  const router = renderPath("/r/rust/new");

  await act(async () => {
    await router.navigate("/r/typescript/comments/def456/title");
  });

  expect(await screen.findByRole("heading", { name: "A TypeScript post" })).toBeVisible();
  expect(
    within(screen.getByRole("complementary")).getByRole("link", { name: "r/typescript" }),
  ).toHaveAttribute("href", "/r/typescript");
  expect(screen.queryByRole("heading", { name: "r/rust" })).not.toBeInTheDocument();
});

test("When a post response arrives after navigating away then it should not replace the current post", async () => {
  const first = Promise.withResolvers<Response>();
  const second = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/posts/:id", ({ params }) =>
      params.id === POST_RESPONSE.id ? first.promise : second.promise,
    ),
  );
  const router = renderPath(POST_PATH);
  await within(await screen.findByRole("region", { name: "Page content" })).findByRole("status");

  await act(async () => {
    await router.navigate("/r/rust/comments/def456");
    second.resolve(HttpResponse.json({ ...POST_RESPONSE, id: "def456", title: "Current post" }));
  });
  await screen.findByRole("heading", { name: "Current post" });
  await act(async () => first.resolve(HttpResponse.json(POST_RESPONSE)));

  expect(screen.getByRole("heading", { name: "Current post" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: POST_RESPONSE.title })).not.toBeInTheDocument();
});

test("When a post does not exist then its not-found state should leave the page chrome visible", async () => {
  renderPath("/r/rust/comments/missing");

  expect(await screen.findByRole("alert")).toHaveTextContent("Post not found");
  expect(screen.getByRole("banner")).toBeVisible();
  expect(screen.getByRole("complementary")).toBeVisible();
});

test("When the post request fails then its error should stay inside the post region", async () => {
  server.use(
    http.get("*/api/posts/:id", () =>
      HttpResponse.json(
        { type: "about:blank", title: "Service Unavailable", status: 503 },
        { status: 503, headers: { "Content-Type": "application/problem+json" } },
      ),
    ),
  );

  renderPath(POST_PATH);

  expect(await screen.findByRole("alert")).toHaveTextContent("Unable to load post");
  expect(screen.getByRole("banner")).toBeVisible();
  expect(screen.getByRole("complementary")).toBeVisible();
});

test("When a post has no Markdown body then its title should still render", async () => {
  server.use(
    http.get("*/api/posts/:id", () => HttpResponse.json({ ...POST_RESPONSE, bodyMarkdown: null })),
  );

  renderPath(POST_PATH);

  expect(await screen.findByRole("heading", { name: POST_RESPONSE.title })).toBeVisible();
  expect(screen.queryByText("Markdown")).not.toBeInTheDocument();
});

test("When posts are loading then the header and subreddit sidebar should remain visible", async () => {
  const pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/subreddits/:subreddit/posts", () => pending.promise));

  renderPath("/r/rust");

  expect(
    await within(await screen.findByRole("region", { name: "Page content" })).findByRole("status"),
  ).toHaveTextContent("Loading posts");
  const header = screen.getByRole("banner");
  const sidebar = screen.getByRole("complementary");
  expect(within(sidebar).getByRole("heading", { name: "r/rust" })).toBeVisible();

  await act(async () => pending.resolve(HttpResponse.json(SUBREDDIT_PAGE_RESPONSE)));

  expect(await screen.findByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Page content" })).queryByRole("status"),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("complementary")).toBe(sidebar);
});

test("When listing options change then only relevant parameters should trigger a request", async () => {
  const requests: URL[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      requests.push(new URL(request.url));
      return HttpResponse.json({
        ...SUBREDDIT_PAGE_RESPONSE,
        posts: [{ ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, title: `Listing ${requests.length}` }],
      });
    }),
  );
  const queryClient = createQueryClient();
  const router = renderPath("/r/rust", queryClient);
  await screen.findByRole("link", { name: "Listing 1" });
  const sidebar = screen.getByRole("complementary");

  await act(async () => {
    await router.navigate("/r/rust?sort=hot&t=week&utm_source=example");
  });

  expect(requests).toHaveLength(1);
  expect(screen.getByRole("link", { name: "Listing 1" })).toBeVisible();

  await act(async () => {
    await router.navigate("/r/rust?sort=top&t=week&utm_source=example");
  });

  expect(await screen.findByRole("link", { name: "Listing 2" })).toBeVisible();
  expect(requests[1]?.searchParams.get("sort")).toBe("top");
  expect(requests[1]?.searchParams.get("timeRange")).toBe("week");
  expect(requests[1]?.searchParams.has("t")).toBe(false);
  expect(requests[1]?.searchParams.has("utm_source")).toBe(false);
  expect(screen.getByRole("complementary")).toBe(sidebar);

  await act(async () => {
    await router.navigate("/r/rust?sort=top&t=month");
  });

  expect(await screen.findByRole("link", { name: "Listing 3" })).toBeVisible();
  expect(requests[2]?.searchParams.get("timeRange")).toBe("month");

  await act(async () => {
    await queryClient.invalidateQueries({
      queryKey: listSubredditPostsQueryKey({
        path: { subreddit: "rust" },
        query: { sort: "top", timeRange: "month" },
      }),
    });
  });

  expect(await screen.findByRole("link", { name: "Listing 4" })).toBeVisible();
  expect(requests).toHaveLength(4);
});

test("When a continuation cursor is supplied then it should reach the API unchanged", async () => {
  const requests: URL[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      requests.push(new URL(request.url));
      return HttpResponse.json({ posts: [], nextCursor: null });
    }),
  );
  const cursor = SUBREDDIT_PAGE_RESPONSE.nextCursor;

  renderPath(`/r/rust?after=${encodeURIComponent(cursor)}`);

  expect(await screen.findByText("No posts")).toBeVisible();
  expect(requests[0]?.searchParams.get("after")).toBe(cursor);
  expect(requests[0]?.searchParams.get("sort")).toBe("hot");
});

test.each([
  [404, "Subreddit not found"],
  [503, "Unable to load posts"],
] as const)(
  "When the listing returns %s then it should show %s within the content region",
  async (status, title) => {
    server.use(
      http.get("*/api/subreddits/:subreddit/posts", () =>
        HttpResponse.json(
          { type: "about:blank", title, status },
          { status, headers: { "Content-Type": "application/problem+json" } },
        ),
      ),
    );

    renderPath("/r/rust");

    expect(await screen.findByRole("alert")).toHaveTextContent(title);
    expect(screen.getByRole("banner")).toBeVisible();
    expect(screen.getByRole("complementary")).toBeVisible();
  },
);

test("When an earlier listing finishes after navigation then it should not replace the current subreddit", async () => {
  const first = Promise.withResolvers<Response>();
  const second = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ params }) =>
      params.subreddit === "rust" ? first.promise : second.promise,
    ),
  );
  const router = renderPath("/r/rust");
  await within(await screen.findByRole("region", { name: "Page content" })).findByRole("status");

  await act(async () => {
    await router.navigate("/r/typescript");
    second.resolve(
      HttpResponse.json({
        ...SUBREDDIT_PAGE_RESPONSE,
        posts: [{ ...SUBREDDIT_PAGE_RESPONSE.posts[0]!, title: "Current listing" }],
      }),
    );
  });
  await screen.findByRole("link", { name: "Current listing" });
  await act(async () => first.resolve(HttpResponse.json(SUBREDDIT_PAGE_RESPONSE)));

  expect(screen.getByRole("link", { name: "Current listing" })).toHaveAttribute(
    "href",
    `/r/typescript/comments/${POST_RESPONSE.id}`,
  );
  expect(screen.queryByRole("link", { name: POST_RESPONSE.title })).not.toBeInTheDocument();
  expect(
    within(screen.getByRole("complementary")).getByRole("link", { name: "r/typescript" }),
  ).toBeVisible();
});

test.each([
  ["/r/rust/hot", "hot", null],
  ["/r/rust/new/", "new", null],
  ["/r/rust/top?t=week", "top", "week"],
  ["/r/rust/top?sort=new", "new", null],
] as const)(
  "When %s is opened then it should redirect before requesting the listing",
  async (path, sort, timeRange) => {
    const requests: URL[] = [];
    server.use(
      http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
        requests.push(new URL(request.url));
        return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
      }),
    );

    const router = renderPath(path);

    await screen.findByRole("link", { name: POST_RESPONSE.title });
    expect(router.state.location.pathname).toBe("/r/rust");
    expect(new URLSearchParams(router.state.location.search).get("sort")).toBe(sort);
    expect(requests).toHaveLength(1);
    expect(requests[0]?.searchParams.get("sort")).toBe(sort);
    expect(requests[0]?.searchParams.get("timeRange")).toBe(timeRange);
    expect(router.state.historyAction).toBe("REPLACE");
  },
);

test("When a sort path redirects then it should preserve the cursor and replace its history entry", async () => {
  const router = renderPath("/");
  const cursor = SUBREDDIT_PAGE_RESPONSE.nextCursor;

  await act(async () => {
    await router.navigate(
      `/r/rust/top?t=week&after=${encodeURIComponent(cursor)}&utm_source=example`,
    );
  });

  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });
  const query = new URLSearchParams(router.state.location.search);
  expect(query.get("sort")).toBe("top");
  expect(query.get("t")).toBe("week");
  expect(query.get("after")).toBe(cursor);
  expect(query.get("utm_source")).toBe("example");

  await act(async () => {
    await router.navigate(-1);
  });

  expect(router.state.location.pathname).toBe("/");
  expect(screen.getByRole("textbox", { name: "r/" })).toBeVisible();
});

test("When listing filters render then they should be connected outline groups with the URL selection active", async () => {
  renderPath("/r/rust?sort=top&t=week");

  const sort = await screen.findByRole("radiogroup", { name: "Sort" });
  const timeRange = screen.getByRole("radiogroup", { name: "Time range" });
  for (const group of [sort, timeRange]) {
    expect(group).toHaveAttribute("data-variant", "outline");
    expect(group).toHaveAttribute("data-spacing", "0");
    for (const option of within(group).getAllByRole("radio")) {
      expect(option).toHaveAttribute("data-variant", "outline");
      expect(option).toHaveAttribute("data-size", "default");
    }
  }
  expect(within(sort).getByRole("radio", { name: "Top" })).toBeChecked();
  expect(within(sort).getByRole("radio", { name: "Hot" })).not.toBeChecked();
  expect(within(timeRange).getByRole("radio", { name: "Week" })).toBeChecked();
  expect(within(timeRange).getByRole("radio", { name: "All time" })).not.toBeChecked();
  expect(timeRange).not.toHaveClass("flex-wrap");
  expect(timeRange.parentElement).toBe(sort.parentElement);
  expect(sort.parentElement).toHaveClass("flex", "overflow-x-auto");
  expect(sort.parentElement).not.toHaveClass("flex-col", "flex-wrap");
});

test("When the selected sort is pressed again then it should remain selected without refetching", async () => {
  const user = userEvent.setup();
  let requests = 0;
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", () => {
      requests++;
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });

  await user.click(screen.getByRole("radio", { name: "Hot" }));

  expect(screen.getByRole("radio", { name: "Hot" })).toBeChecked();
  expect(screen.queryByRole("radio", { name: "All time" })).not.toBeInTheDocument();
  expect(router.state.location.search).toBe("");
  expect(requests).toBe(1);
});

test("When the sort changes on a later page then it should clear the cursor and irrelevant time range", async () => {
  const user = userEvent.setup();
  const requests: URL[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      requests.push(new URL(request.url));
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  const router = renderPath("/r/rust?sort=top&t=week&after=cursor&utm_source=example");
  await screen.findByRole("link", { name: POST_RESPONSE.title });

  await user.click(screen.getByRole("radio", { name: "New" }));

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=new&utm_source=example");
  expect(screen.getByRole("radio", { name: "New" })).toBeChecked();
  expect(screen.queryByRole("radio", { name: "Week" })).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "First page" })).not.toBeInTheDocument();
  expect(requests).toHaveLength(2);
  expect(requests[1]?.search).toBe("?sort=new");
});

test.each([
  ["Hour", "hour"],
  ["Day", "day"],
  ["Week", "week"],
  ["Month", "month"],
  ["Year", "year"],
  ["All time", "all"],
])(
  "When %s is selected then the URL and request should use %s without the old cursor",
  async (label, timeRange) => {
    const user = userEvent.setup();
    const requests: URL[] = [];
    server.use(
      http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
        requests.push(new URL(request.url));
        return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
      }),
    );
    const router = renderPath(
      `/r/rust?sort=top&t=${timeRange === "all" ? "week" : "all"}&after=cursor`,
    );
    await screen.findByRole("link", { name: POST_RESPONSE.title });

    await user.click(screen.getByRole("radio", { name: label }));

    await screen.findByRole("link", { name: POST_RESPONSE.title });
    expect(router.state.location.search).toBe(`?sort=top&t=${timeRange}`);
    expect(screen.getByRole("radio", { name: label })).toBeChecked();
    expect(requests).toHaveLength(2);
    expect(requests[1]?.search).toBe(`?sort=top&timeRange=${timeRange}`);
  },
);

test("When Next and First page are followed then paging should preserve the filters in the URL", async () => {
  const user = userEvent.setup();
  const router = renderPath("/r/rust?sort=top&t=week&utm_source=example");
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  const next = screen.getByRole("link", { name: "Next" });
  expect(next).toHaveClass("border", "border-border");
  expect(next).not.toHaveClass("border-transparent");
  const nextQuery = new URL(next.getAttribute("href")!, "http://localhost").searchParams;
  expect(nextQuery.get("sort")).toBe("top");
  expect(nextQuery.get("t")).toBe("week");
  expect(nextQuery.get("after")).toBe(SUBREDDIT_PAGE_RESPONSE.nextCursor);
  expect(screen.queryByRole("link", { name: "First page" })).not.toBeInTheDocument();

  await user.click(next);

  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });
  expect(screen.queryByRole("link", { name: POST_RESPONSE.title })).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "Next" })).not.toBeInTheDocument();
  expect(new URLSearchParams(router.state.location.search).get("after")).toBe(
    SUBREDDIT_PAGE_RESPONSE.nextCursor,
  );

  const first = screen.getByRole("link", { name: "First page" });
  expect(first).toHaveClass("border", "border-border");
  expect(first).not.toHaveClass("border-transparent");
  await user.click(first);

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=top&t=week&utm_source=example");
  expect(screen.queryByRole("link", { name: "First page" })).not.toBeInTheDocument();
});

test("When a cursor contains reserved characters then Next should pass it to the API unchanged", async () => {
  const user = userEvent.setup();
  const cursor = "opaque+/=& ?#";
  const requests: URL[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      const url = new URL(request.url);
      requests.push(url);
      return HttpResponse.json(
        url.searchParams.has("after")
          ? SUBREDDIT_NEXT_PAGE_RESPONSE
          : { ...SUBREDDIT_PAGE_RESPONSE, nextCursor: cursor },
      );
    }),
  );
  const router = renderPath("/r/rust?sort=new");
  await screen.findByRole("link", { name: "Next" });

  await user.click(screen.getByRole("link", { name: "Next" }));

  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });
  expect(new URLSearchParams(router.state.location.search).get("after")).toBe(cursor);
  expect(requests).toHaveLength(2);
  expect(requests[1]?.searchParams.get("after")).toBe(cursor);
  expect(requests[1]?.searchParams.get("sort")).toBe("new");
});

test("When an empty final page is opened directly then First page should return to the listing", async () => {
  const user = userEvent.setup();
  const router = renderPath("/r/rust?after=empty");
  await screen.findByText("No posts");
  expect(screen.queryByRole("link", { name: "Next" })).not.toBeInTheDocument();

  await user.click(screen.getByRole("link", { name: "First page" }));

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.pathname).toBe("/r/rust");
  expect(router.state.location.search).toBe("");
});

test("When the first page has no continuation then pagination should not be shown", async () => {
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", () =>
      HttpResponse.json({ posts: [], nextCursor: null }),
    ),
  );

  renderPath("/r/rust");

  await screen.findByText("No posts");
  expect(screen.queryByRole("navigation", { name: "Pagination" })).not.toBeInTheDocument();
});

test("When browser history changes then controls and posts should follow the URL", async () => {
  const user = userEvent.setup();
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });

  await user.click(screen.getByRole("radio", { name: "Top" }));
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(screen.getByRole("radio", { name: "All time" })).toBeChecked();
  await user.click(screen.getByRole("radio", { name: "Week" }));
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  await user.click(screen.getByRole("link", { name: "Next" }));
  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });

  await act(async () => {
    await router.navigate(-1);
  });

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=top&t=week");
  expect(screen.getByRole("radio", { name: "Week" })).toBeChecked();
  expect(screen.queryByRole("link", { name: "First page" })).not.toBeInTheDocument();

  await act(async () => {
    await router.navigate(-1);
  });

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=top");
  expect(screen.getByRole("radio", { name: "All time" })).toBeChecked();

  await act(async () => {
    await router.navigate(1);
  });

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=top&t=week");
  expect(screen.getByRole("radio", { name: "Week" })).toBeChecked();
});

test("When a later page is pending then sorting should remain usable while the old listing and disabled pagination stay visible", async () => {
  const user = userEvent.setup();
  const pending = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) =>
      new URL(request.url).searchParams.has("after")
        ? pending.promise
        : HttpResponse.json(SUBREDDIT_PAGE_RESPONSE),
    ),
  );
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: "Next" });

  await user.click(screen.getByRole("link", { name: "Next" }));

  const content = screen.getByRole("region", { name: "Page content" });
  expect(within(content).queryByRole("status")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(content.querySelector("[aria-busy]")).toHaveAttribute("aria-busy", "true");
  expect(content.querySelector("[aria-busy]")).toHaveClass("opacity-80");
  expect(screen.getByRole("link", { name: "Next" })).toHaveAttribute("aria-disabled", "true");
  expect(screen.getByRole("link", { name: "Next" })).not.toHaveAttribute("href");

  await user.click(screen.getByRole("radio", { name: "New" }));
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  await waitFor(() =>
    expect(content.querySelector("[aria-busy]")).toHaveAttribute("aria-busy", "false"),
  );
  await act(async () => {
    pending.resolve(HttpResponse.json(SUBREDDIT_NEXT_PAGE_RESPONSE));
  });

  expect(router.state.location.search).toBe("?sort=new");
  expect(
    screen.queryByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("radio", { name: "New" })).toBeChecked();
});

test("When the current filters are selected on a later page then the cursor should remain unchanged", async () => {
  const user = userEvent.setup();
  const cursor = SUBREDDIT_PAGE_RESPONSE.nextCursor;
  const path = `/r/rust?sort=top&t=week&after=${encodeURIComponent(cursor)}`;
  const router = renderPath(path);
  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });

  await user.click(screen.getByRole("radio", { name: "Top" }));
  await user.click(screen.getByRole("radio", { name: "Week" }));

  expect(`${router.state.location.pathname}${router.state.location.search}`).toBe(path);
  expect(
    screen.getByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title }),
  ).toBeVisible();
});

test("When keyboard navigation changes the sort then the URL should follow the selection", async () => {
  const user = userEvent.setup();
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  screen.getByRole("radio", { name: "Hot" }).focus();

  await user.keyboard("{ArrowRight}");
  expect(screen.getByRole("radio", { name: "New" })).toHaveFocus();
  await user.keyboard(" ");

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=new");
  expect(screen.getByRole("radio", { name: "New" })).toBeChecked();
});

test("When a continuation request fails then First page should recover without changing filters", async () => {
  const user = userEvent.setup();
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) =>
      new URL(request.url).searchParams.has("after")
        ? HttpResponse.json(
            { type: "about:blank", title: "Bad Request", status: 400 },
            { status: 400 },
          )
        : HttpResponse.json(SUBREDDIT_PAGE_RESPONSE),
    ),
  );
  const router = renderPath("/r/rust?sort=top&t=week&after=invalid");
  expect(await screen.findByRole("alert")).toHaveTextContent("Unable to load posts");
  expect(screen.queryByRole("link", { name: "Next" })).not.toBeInTheDocument();

  await user.click(screen.getByRole("link", { name: "First page" }));

  await screen.findByRole("link", { name: POST_RESPONSE.title });
  expect(router.state.location.search).toBe("?sort=top&t=week");
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("When navigating away and back then completed post and listing queries should be reused", async () => {
  const requests: string[] = [];
  server.use(
    http.get("*/api/posts/:id", ({ params, request }) => {
      requests.push(new URL(request.url).pathname);
      return HttpResponse.json({
        ...POST_RESPONSE,
        id: String(params.id),
        title: `Post ${params.id}`,
      });
    }),
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      requests.push(new URL(request.url).pathname);
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });

  await act(async () => {
    await router.navigate(POST_PATH);
  });
  await screen.findByRole("heading", { name: `Post ${POST_RESPONSE.id}` });
  await act(async () => {
    await router.navigate("/r/rust/comments/def456");
  });
  await screen.findByRole("heading", { name: "Post def456" });
  await act(async () => {
    await router.navigate(-1);
  });

  expect(screen.getByRole("heading", { name: `Post ${POST_RESPONSE.id}` })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Page content" })).queryByRole("status"),
  ).not.toBeInTheDocument();

  await act(async () => {
    await router.navigate(-1);
  });

  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Page content" })).queryByRole("status"),
  ).not.toBeInTheDocument();
  expect(requests).toEqual([
    "/api/subreddits/rust/posts",
    `/api/posts/${POST_RESPONSE.id}`,
    "/api/posts/def456",
  ]);
});

test("When returning to an earlier sort or page then its cached result should not be requested again", async () => {
  const user = userEvent.setup();
  const requests: URL[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit/posts", ({ request }) => {
      const url = new URL(request.url);
      requests.push(url);
      return HttpResponse.json(
        url.searchParams.has("after") ? SUBREDDIT_NEXT_PAGE_RESPONSE : SUBREDDIT_PAGE_RESPONSE,
      );
    }),
  );
  const router = renderPath("/r/rust");
  await screen.findByRole("link", { name: POST_RESPONSE.title });

  await user.click(screen.getByRole("link", { name: "Next" }));
  await screen.findByRole("link", { name: SUBREDDIT_NEXT_PAGE_RESPONSE.posts[0]!.title });
  await user.click(screen.getByRole("link", { name: "First page" }));

  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(requests).toHaveLength(2);

  await user.click(screen.getByRole("radio", { name: "New" }));
  await screen.findByRole("link", { name: POST_RESPONSE.title });
  await act(async () => {
    await router.navigate(-1);
  });

  expect(router.state.location.search).toBe("");
  expect(screen.getByRole("radio", { name: "Hot" })).toBeChecked();
  expect(screen.getByRole("link", { name: POST_RESPONSE.title })).toBeVisible();
  expect(
    within(screen.getByRole("region", { name: "Page content" })).queryByRole("status"),
  ).not.toBeInTheDocument();
  expect(requests).toHaveLength(3);
  expect(requests[2]?.searchParams.get("sort")).toBe("new");
});

test("When a pending post is left behind then its generated query should abort the request", async () => {
  const started = Promise.withResolvers<void>();
  const aborted = Promise.withResolvers<void>();
  const pending = Promise.withResolvers<Response>();
  server.use(
    http.get("*/api/posts/:id", ({ request }) => {
      request.signal.addEventListener("abort", () => aborted.resolve());
      started.resolve();
      return pending.promise;
    }),
  );
  const router = renderPath(POST_PATH);
  await started.promise;

  await act(async () => {
    await router.navigate("/");
  });
  await aborted.promise;
  await act(async () => {
    pending.resolve(HttpResponse.json(POST_RESPONSE));
  });

  expect(screen.getByRole("textbox", { name: "r/" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: POST_RESPONSE.title })).not.toBeInTheDocument();
});
test.each(CONTENT_REQUESTS)(
  "When metadata finishes before the %s then only the content region should remain loading",
  async (_, path, endpoint, response) => {
    const content = Promise.withResolvers<Response>();
    const metadata = Promise.withResolvers<Response>();
    const contentStarted = Promise.withResolvers<void>();
    const metadataStarted = Promise.withResolvers<void>();
    server.use(
      http.get(endpoint, () => {
        contentStarted.resolve();
        return content.promise;
      }),
      http.get("*/api/subreddits/:subreddit", () => {
        metadataStarted.resolve();
        return metadata.promise;
      }),
    );
    renderPath(path);
    await Promise.all([contentStarted.promise, metadataStarted.promise]);
    const sidebar = screen.getByRole("complementary");
    const region = screen.getByRole("region", { name: "Page content" });
    expect(within(sidebar).getByRole("status")).toHaveTextContent("Loading subreddit details");

    await act(async () => {
      metadata.resolve(HttpResponse.json(SUBREDDIT_RESPONSE));
    });

    expect(
      await within(sidebar).findByText(SUBREDDIT_RESPONSE.title, { selector: "p" }),
    ).toBeVisible();
    expect(within(sidebar).queryByRole("status")).not.toBeInTheDocument();
    expect(within(region).getByRole("status")).toBeVisible();

    await act(async () => {
      content.resolve(HttpResponse.json(response));
    });

    expect(await within(region).findByText(POST_RESPONSE.title)).toBeVisible();
    expect(within(region).queryByRole("status")).not.toBeInTheDocument();
  },
);

test.each(CONTENT_REQUESTS)(
  "When the %s finishes before metadata then the sidebar should not block it",
  async (_, path) => {
    const pending = Promise.withResolvers<Response>();
    server.use(http.get("*/api/subreddits/:subreddit", () => pending.promise));

    renderPath(path);

    const region = await screen.findByRole("region", { name: "Page content" });
    expect(await within(region).findByText(POST_RESPONSE.title)).toBeVisible();
    const sidebar = screen.getByRole("complementary");
    expect(within(sidebar).getByRole("status")).toHaveTextContent("Loading subreddit details");
    expect(within(sidebar).getByRole("link", { name: "r/rust" })).toBeVisible();
    expect(sidebar).toHaveClass("select-text");

    await act(async () => {
      pending.resolve(HttpResponse.json(SUBREDDIT_RESPONSE));
    });

    expect(
      await within(sidebar).findByText(SUBREDDIT_RESPONSE.title, { selector: "p" }),
    ).toBeVisible();
    expect(within(region).getByText(POST_RESPONSE.title)).toBeVisible();
  },
);

test.each([
  [404, "Subreddit details not found"],
  [503, "Unable to load subreddit details"],
] as const)(
  "When metadata returns %s then its %s state should stay in the sidebar",
  async (status, title) => {
    server.use(
      http.get("*/api/subreddits/:subreddit", () =>
        HttpResponse.json({ type: "about:blank", title, status }, { status }),
      ),
    );

    renderPath("/r/rust");

    const sidebar = await screen.findByRole("complementary");
    expect(await within(sidebar).findByRole("alert")).toHaveTextContent(title);
    expect(within(sidebar).getByRole("link", { name: "r/rust" })).toBeVisible();
    expect(
      await within(screen.getByRole("region", { name: "Page content" })).findByRole("link", {
        name: POST_RESPONSE.title,
      }),
    ).toBeVisible();
  },
);

test.each(CONTENT_REQUESTS)(
  "When the %s request fails then successful metadata should remain visible",
  async (_, path, endpoint) => {
    server.use(
      http.get(endpoint, () =>
        HttpResponse.json(
          { type: "about:blank", title: "Unavailable", status: 503 },
          { status: 503 },
        ),
      ),
    );

    renderPath(path);

    const sidebar = await screen.findByRole("complementary");
    expect(
      await within(sidebar).findByText(SUBREDDIT_RESPONSE.title, { selector: "p" }),
    ).toBeVisible();
    expect(
      await within(screen.getByRole("region", { name: "Page content" })).findByRole("alert"),
    ).toBeVisible();
    expect(within(sidebar).queryByRole("alert")).not.toBeInTheDocument();
  },
);

test("When metadata reports an unavailable community with HTTP 200 then its explanation should not hide a readable post", async () => {
  server.use(
    http.get("*/api/subreddits/:subreddit", () =>
      HttpResponse.json({
        kind: "unavailable",
        id: null,
        banTitle: "Community unavailable",
        banMessageMarkdown: "The community is **restricted**.",
        forbiddenReason: null,
        interstitialWarningMarkdown: null,
        quarantineMessageMarkdown: null,
      }),
    ),
  );

  renderPath(POST_PATH);

  const sidebar = await screen.findByRole("complementary");
  expect(await within(sidebar).findByRole("alert")).toHaveTextContent("Community unavailable");
  expect(within(sidebar).getByText("restricted").tagName).toBe("STRONG");
  expect(
    await within(screen.getByRole("region", { name: "Page content" })).findByRole("heading", {
      name: POST_RESPONSE.title,
    }),
  ).toBeVisible();
});

test("When navigating within a subreddit and returning to it then metadata should be reused independently of post and listing keys", async () => {
  const metadataRequests: string[] = [];
  server.use(
    http.get("*/api/subreddits/:subreddit", ({ params }) => {
      metadataRequests.push(String(params.subreddit));
      return HttpResponse.json({
        ...SUBREDDIT_RESPONSE,
        name: String(params.subreddit),
        title: `About r/${params.subreddit}`,
        descriptionMarkdown: null,
      });
    }),
    http.get("*/api/posts/:id", ({ params }) =>
      HttpResponse.json({ ...POST_RESPONSE, id: String(params.id), title: `Post ${params.id}` }),
    ),
  );
  const router = renderPath("/r/rust");
  const sidebar = await screen.findByRole("complementary");
  await within(sidebar).findByText("About r/rust");

  for (const path of [
    "/r/rust?sort=new",
    "/r/rust?sort=top&t=week",
    `/r/rust?after=${SUBREDDIT_PAGE_RESPONSE.nextCursor}`,
    POST_PATH,
    "/r/rust/comments/def456",
  ]) {
    await act(async () => {
      await router.navigate(path);
    });
    expect(screen.getByRole("complementary")).toBe(sidebar);
    expect(within(sidebar).getByText("About r/rust")).toBeVisible();
    expect(within(sidebar).queryByRole("status")).not.toBeInTheDocument();
  }
  expect(metadataRequests).toEqual(["rust"]);

  await act(async () => {
    await router.navigate("/r/typescript");
  });
  await within(sidebar).findByText("About r/typescript");
  await act(async () => {
    await router.navigate("/");
  });
  await act(async () => {
    await router.navigate("/r/rust");
  });

  expect(within(screen.getByRole("complementary")).getByText("About r/rust")).toBeVisible();
  expect(within(screen.getByRole("complementary")).queryByRole("status")).not.toBeInTheDocument();
  expect(metadataRequests).toEqual(["rust", "typescript"]);
});

test("When an earlier metadata response arrives after switching subreddits then it should not replace the current sidebar", async () => {
  const pending = Promise.withResolvers<Response>();
  const started = Promise.withResolvers<void>();
  server.use(
    http.get("*/api/subreddits/:subreddit", ({ params }) => {
      if (params.subreddit === "rust") {
        started.resolve();
        return pending.promise;
      }
      return HttpResponse.json({
        ...SUBREDDIT_RESPONSE,
        name: "typescript",
        title: "TypeScript metadata",
        descriptionMarkdown: null,
      });
    }),
  );
  const router = renderPath("/r/rust");
  await started.promise;

  await act(async () => {
    await router.navigate("/r/typescript");
  });
  const sidebar = screen.getByRole("complementary");
  await within(sidebar).findByText("TypeScript metadata");
  await act(async () => {
    pending.resolve(HttpResponse.json(SUBREDDIT_RESPONSE));
  });

  expect(within(sidebar).getByRole("link", { name: "r/typescript" })).toHaveAttribute(
    "href",
    "/r/typescript",
  );
  expect(within(sidebar).getByText("TypeScript metadata")).toBeVisible();
  expect(within(sidebar).queryByText(SUBREDDIT_RESPONSE.title)).not.toBeInTheDocument();
});
