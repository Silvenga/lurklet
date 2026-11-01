import { QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { http, HttpResponse } from "msw/http";
import { createMemoryRouter, parsePath } from "react-router";
import { RouterProvider } from "react-router/dom";
import { beforeEach, expect, onTestFinished, test, vi } from "vitest";
import { POST_RESPONSE, SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { DeferredScrollRestoration } from "./DeferredScrollRestoration";
import { createQueryClient } from "./query-client";
import { APP_ROUTES } from "./router";

let scrollY = 0;
let maximumScroll = Infinity;
const resizeCallbacks = new Set<() => void>();

beforeEach(() => {
  scrollY = 0;
  maximumScroll = Infinity;
  resizeCallbacks.clear();
  vi.spyOn(window, "scrollY", "get").mockImplementation(() => scrollY);
  vi.spyOn(window, "scrollTo").mockImplementation((_x, y) => {
    scrollY = Math.min(maximumScroll, y);
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      private callback: () => void;
      constructor(callback: () => void) {
        this.callback = callback;
      }
      observe() {
        resizeCallbacks.add(this.callback);
      }
      disconnect() {
        resizeCallbacks.delete(this.callback);
      }
      unobserve() {
        this.disconnect();
      }
    },
  );
  onTestFinished(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });
});

function renderPath(path: string) {
  const queryClient = createQueryClient();
  const router = createMemoryRouter(APP_ROUTES, {
    initialEntries: [{ ...parsePath(path), key: crypto.randomUUID() }],
  });
  const { unmount } = render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <DeferredScrollRestoration router={router} />
    </QueryClientProvider>,
  );
  onTestFinished(() => {
    unmount();
    router.dispose();
    queryClient.clear();
  });
  return { router, queryClient };
}

test("When navigating and traversing history then each entry should reset or restore its own scroll position", async () => {
  const { router } = renderPath("/r/rust");
  await screen.findByRole("link", { name: "Next" });
  window.scrollTo(0, 600);

  await act(async () => {
    await router.navigate("/r/rust?after=next");
  });

  expect(window.scrollY).toBe(0);
  window.scrollTo(0, 200);

  await act(async () => {
    await router.navigate(`/r/rust/comments/${POST_RESPONSE.id}`);
  });

  expect(window.scrollY).toBe(0);
  window.scrollTo(0, 900);

  await act(async () => {
    await router.navigate(-1);
  });

  expect(window.scrollY).toBe(200);

  await act(async () => {
    await router.navigate(-1);
  });

  expect(window.scrollY).toBe(600);

  await act(async () => {
    await router.navigate(1);
  });

  expect(window.scrollY).toBe(200);

  await act(async () => {
    await router.navigate("/r/rust");
  });

  expect(window.scrollY).toBe(0);
});

test.each([
  { path: "/r/rust", control: "New" },
  { path: "/r/rust?sort=top&t=week", control: "Month" },
  { path: `/r/rust/comments/${POST_RESPONSE.id}`, control: "New" },
])(
  "When changing $path using $control then the current scroll position should be preserved",
  async ({ path, control }) => {
    const { router } = renderPath(path);
    const toggle = await screen.findByRole("radio", { name: control });
    window.scrollTo(0, 350);

    await act(async () => {
      fireEvent.click(toggle);
    });

    expect(router.state.location.search).toContain(control === "Month" ? "t=month" : "sort=new");
    expect(window.scrollY).toBe(350);
  },
);

test("When returning to an uncached listing then restoration should retry until the loaded content reaches the saved position", async () => {
  const { router, queryClient } = renderPath("/r/rust");
  await screen.findByRole("link", { name: "Next" });
  window.scrollTo(0, 600);
  await act(async () => {
    await router.navigate("/");
  });
  queryClient.clear();
  const pending = Promise.withResolvers<Response>();
  server.use(http.get("*/api/subreddits/:subreddit/posts", () => pending.promise));
  maximumScroll = 100;

  await act(async () => {
    await router.navigate(-1);
  });

  expect(window.scrollY).toBe(100);
  expect(screen.getByText("Loading posts")).toBeInTheDocument();
  for (const callback of resizeCallbacks) callback();
  expect(window.scrollY).toBe(100);

  await act(async () => {
    pending.resolve(HttpResponse.json(SUBREDDIT_PAGE_RESPONSE));
  });
  await screen.findByRole("link", { name: "Next" });
  maximumScroll = 1000;
  for (const callback of resizeCallbacks) callback();

  expect(window.scrollY).toBe(600);
  expect(resizeCallbacks.size).toBe(0);
});

test.each(["interaction", "navigation"])(
  "When deferred restoration is interrupted by %s then later resizing should not move the page",
  async (interruption) => {
    const { router } = renderPath("/r/rust");
    await screen.findByRole("link", { name: "Next" });
    window.scrollTo(0, 600);
    await act(async () => {
      await router.navigate("/");
    });
    maximumScroll = 100;
    await act(async () => {
      await router.navigate(-1);
    });
    expect(resizeCallbacks.size).toBe(1);

    if (interruption === "interaction") {
      fireEvent.wheel(window);
    } else {
      await act(async () => {
        await router.navigate("/");
      });
    }
    const interruptedPosition = window.scrollY;
    maximumScroll = 1000;
    for (const callback of resizeCallbacks) callback();

    expect(resizeCallbacks.size).toBe(0);
    expect(window.scrollY).toBe(interruptedPosition);
  },
);
