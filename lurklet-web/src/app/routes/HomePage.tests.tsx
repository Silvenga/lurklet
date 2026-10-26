import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw/http";
import { createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test } from "vitest";
import { SUBREDDIT_RESPONSE, SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import { server } from "@/_tests/mocks/server";
import { createQueryClient } from "../query-client";
import { APP_ROUTES } from "../router";

const SUGGESTIONS = [
  ["aww", "Cute animals and other adorable things"],
  ["wholesomememes", "Feel-good memes"],
  ["ContagiousLaughter", "Videos of infectious laughter"],
  ["MadeMeSmile", "Moments that brighten someone's day"],
  ["AnimalsBeingDerps", "Silly animals"],
] as const;

function renderHome() {
  const router = createMemoryRouter(APP_ROUTES);
  const queryClient = createQueryClient();
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

test("When the home page renders then it should show the static introduction", () => {
  renderHome();

  expect(screen.getByRole("heading", { level: 1, name: "Lurklet" })).toBeInTheDocument();
  expect(
    screen.getByText("A lightweight, privacy-focused front end for Reddit."),
  ).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "r/" })).toHaveClass("select-text");
  expect(screen.getByRole("button", { name: "Go" })).toHaveAttribute("type", "submit");
});

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
test("When the home page renders then the suggestions should be ordered subreddit links without subscriber counts", () => {
  renderHome();

  const suggestions = screen.getByRole("navigation", { name: "Suggested subreddits" });
  const links = within(suggestions).getAllByRole("link");
  expect(links.map((link) => link.textContent)).toEqual(SUGGESTIONS.map(([name]) => `r/${name}`));
  for (const [name] of SUGGESTIONS) {
    const link = within(suggestions).getByRole("link", { name: `r/${name}` });
    expect(link).toHaveAttribute("href", `/r/${name}`);
    expect(link).not.toHaveAttribute("target");
    expect(link.closest("form")).toBeNull();
  }
  expect(suggestions).not.toHaveTextContent(/\d/);
});

test.each(SUGGESTIONS)(
  "When r/%s is hovered then its tooltip should show %s",
  async (name, description) => {
    const user = userEvent.setup();
    renderHome();

    await user.hover(screen.getByRole("link", { name: `r/${name}` }));

    expect(await screen.findByRole("tooltip")).toHaveTextContent(description);
  },
);

test.each(SUGGESTIONS)("When r/%s is clicked then its posts page should open", async (name) => {
  const user = userEvent.setup();
  const router = renderHome();

  await user.click(screen.getByRole("link", { name: `r/${name}` }));

  expect(await screen.findByRole("heading", { level: 1, name: `r/${name}` })).toBeVisible();
  expect(router.state.location.pathname).toBe(`/r/${name}`);
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
