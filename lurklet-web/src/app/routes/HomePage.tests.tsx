import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw/http";
import { createMemoryRouter, useParams } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test } from "vitest";
import { SUBREDDIT_RESPONSE, SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { HomePage } from "./HomePage";

function renderHome() {
  const router = createMemoryRouter([
    { path: "/", Component: HomePage },
    {
      path: "/r/:subreddit",
      Component: () => {
        const { subreddit } = useParams();
        return <h1>r/{subreddit}</h1>;
      },
    },
  ]);
  const { unmount } = render(<RouterProvider router={router} />);
  onTestFinished(() => {
    unmount();
    router.dispose();
  });
  return router;
}

test("When the submit button is pressed then it should navigate to the entered subreddit", async () => {
  const user = userEvent.setup();
  const router = renderHome();
  await user.type(screen.getByRole("textbox", { name: "r/" }), "rust");

  await user.click(screen.getByRole("button", { name: "Go" }));

  expect(await screen.findByRole("heading", { level: 1, name: "r/rust" })).toBeVisible();
  expect(router.state.location.pathname).toBe("/r/rust");
});

test.each([
  ["typescript", "typescript"],
  ["  Rust_Lang  ", "Rust_Lang"],
])(
  "When Enter is pressed after typing %s then it should navigate to r/%s",
  async (value, subreddit) => {
    const user = userEvent.setup();
    const router = renderHome();

    await user.type(screen.getByRole("textbox", { name: "r/" }), `${value}{Enter}`);

    expect(await screen.findByRole("heading", { level: 1, name: `r/${subreddit}` })).toBeVisible();
    expect(router.state.location.pathname).toBe(`/r/${subreddit}`);
  },
);

test.each(["", "   "])(
  "When the input is %j then submitting should stay on the landing page",
  async (value) => {
    const user = userEvent.setup();
    const router = renderHome();
    if (value) {
      await user.type(screen.getByRole("textbox", { name: "r/" }), value);
    }

    await user.click(screen.getByRole("button", { name: "Go" }));

    expect(router.state.location.pathname).toBe("/");
    expect(screen.getByRole("heading", { level: 1, name: "Lurklet" })).toBeVisible();
  },
);
test("When a suggested subreddit is hovered then its tooltip should describe it", async () => {
  const user = userEvent.setup();
  renderHome();

  await user.hover(document.body);
  await user.hover(screen.getByRole("link", { name: "r/MadeMeSmile" }));

  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    "Moments that brighten someone's day",
  );
});

test("When suggestions are explored with the keyboard then the tooltip should appear without fetching subreddit data", async () => {
  const user = userEvent.setup();
  let requests = 0;
  server.use(
    http.get("*/api/subreddits/:subreddit", () => {
      requests++;
      return HttpResponse.json(SUBREDDIT_RESPONSE);
    }),
    http.get("*/api/subreddits/:subreddit/posts", () => {
      requests++;
      return HttpResponse.json(SUBREDDIT_PAGE_RESPONSE);
    }),
  );
  const router = renderHome();

  screen.getByRole("button", { name: "Go" }).focus();
  await user.tab();

  expect(screen.getByRole("link", { name: "r/aww" })).toHaveFocus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    "Cute animals and other adorable things",
  );
  expect(router.state.location.pathname).toBe("/");
  expect(requests).toBe(0);

  await user.keyboard("{Enter}");

  expect(await screen.findByRole("heading", { level: 1, name: "r/aww" })).toBeVisible();
  expect(router.state.location.pathname).toBe("/r/aww");
});
