import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { Suspense } from "react";
import { Await, createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, test } from "vitest";
import { AppLayout } from "./AppLayout";
import { HomePage } from "./routes/HomePage";

test("When a page renders then the shared header and page content should appear", () => {
  const router = createMemoryRouter([
    {
      path: "/",
      Component: AppLayout,
      children: [{ index: true, element: <h1>Page content</h1> }],
    },
  ]);

  render(<RouterProvider router={router} />);

  expect(within(screen.getByRole("banner")).getByRole("link", { name: "Lurklet" })).toHaveAttribute(
    "href",
    "/",
  );
  const repositoryLink = within(screen.getByRole("banner")).getByRole("link", {
    name: "GitHub repository",
  });
  expect(repositoryLink).toHaveAttribute("href", "https://github.com/Silvenga/lurklet");
  expect(repositoryLink).toHaveAttribute("target", "_blank");
  expect(repositoryLink).toHaveAttribute("rel", "noopener noreferrer");
  expect(screen.getByRole("main")).toHaveClass("pt-2", "pb-4", "sm:py-8");
  expect(
    within(screen.getByRole("main")).getByRole("heading", { name: "Page content" }),
  ).toBeInTheDocument();
});

test("When the home link is activated then navigation should preserve the shared header", async () => {
  const router = createMemoryRouter(
    [
      {
        path: "/",
        Component: AppLayout,
        children: [
          { index: true, Component: HomePage },
          { path: "example", element: <h1>Example page</h1> },
        ],
      },
    ],
    { initialEntries: ["/example"] },
  );
  render(<RouterProvider router={router} />);
  const header = screen.getByRole("banner");

  fireEvent.click(within(header).getByRole("link", { name: "Lurklet" }));

  expect(await screen.findByRole("heading", { name: "Lurklet" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "Example page" })).not.toBeInTheDocument();
  expect(screen.getByRole("banner")).toBe(header);
});

test("When page content suspends then the shared header should remain visible", async () => {
  const content = Promise.withResolvers<string>();
  const router = createMemoryRouter([
    {
      path: "/",
      Component: AppLayout,
      children: [
        {
          index: true,
          element: (
            <Suspense fallback={<p>Loading page content</p>}>
              <Await resolve={content.promise}>{(value: string) => <p>{value}</p>}</Await>
            </Suspense>
          ),
        },
      ],
    },
  ]);

  render(<RouterProvider router={router} />);

  const header = screen.getByRole("banner");
  expect(within(header).getByRole("link", { name: "Lurklet" })).toBeVisible();
  expect(within(screen.getByRole("main")).getByText("Loading page content")).toBeVisible();

  await act(async () => content.resolve("Loaded page content"));

  expect(within(screen.getByRole("main")).getByText("Loaded page content")).toBeVisible();
  expect(screen.queryByText("Loading page content")).not.toBeInTheDocument();
  expect(screen.getByRole("banner")).toBe(header);
});
