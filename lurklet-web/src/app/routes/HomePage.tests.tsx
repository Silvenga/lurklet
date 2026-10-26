import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test } from "vitest";
import { createQueryClient } from "../query-client";
import { APP_ROUTES } from "../router";

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

  expect(await screen.findByRole("heading", { name: "r/rust - hot" })).toBeVisible();
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

    expect(await screen.findByRole("heading", { name: `r/${subreddit} - hot` })).toBeVisible();
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
import { QueryClientProvider } from "@tanstack/react-query";
