import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { createMemoryRouter } from "react-router";
import { RouterProvider } from "react-router/dom";
import { expect, onTestFinished, test, vi } from "vitest";
import { version } from "../../package.json";
import { AppLayout } from "./AppLayout";
import { HomePage } from "./routes/HomePage";

test("When a page renders then the shared header, version footer, and page content should appear", () => {
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
  expect(
    within(screen.getByRole("main")).getByRole("heading", { name: "Page content" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("contentinfo")).toHaveTextContent(`Lurklet v${version}`);
});

test("When the home link is activated then navigation should preserve the shared header and footer", async () => {
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
  const footer = screen.getByRole("contentinfo");

  fireEvent.click(within(header).getByRole("link", { name: "Lurklet" }));

  expect(await screen.findByRole("heading", { name: "Lurklet" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "Example page" })).not.toBeInTheDocument();
  expect(screen.getByRole("banner")).toBe(header);
  expect(screen.getByRole("contentinfo")).toBe(footer);
});

test("When navigating to a fragment then scrolling should target the matching element", async () => {
  const scrollIntoView = vi.spyOn(HTMLElement.prototype, "scrollIntoView");
  const router = createMemoryRouter([
    {
      path: "/",
      Component: AppLayout,
      children: [{ index: true, element: <h1 id="target">Anchor target</h1> }],
    },
  ]);
  const { unmount } = render(<RouterProvider router={router} />);
  onTestFinished(() => {
    unmount();
    router.dispose();
    scrollIntoView.mockRestore();
  });

  await act(async () => {
    await router.navigate("/#target");
  });

  expect(scrollIntoView.mock.contexts).toEqual([
    screen.getByRole("heading", { name: "Anchor target" }),
  ]);
});
