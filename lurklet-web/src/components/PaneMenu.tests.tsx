import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, onTestFinished, test, vi } from "vitest";
import { PaneMenu } from "./PaneMenu";

const RELATIVE_FORMAT = new Intl.RelativeTimeFormat(undefined, { numeric: "always" });

test("When a pane menu opens with the keyboard then its JSON item should be a new-tab link and Escape should restore focus", async () => {
  const user = userEvent.setup();
  const href = new URL("/api/posts/example", location.origin).href;
  render(<PaneMenu jsonHref={href} label="Post actions" />);
  const trigger = screen.getByRole("button", { name: "Post actions" });

  await user.tab();

  expect(trigger).toHaveFocus();
  expect(screen.queryByRole("menuitem")).toBeNull();

  await user.keyboard("{ArrowDown}");

  const link = await screen.findByRole("menuitem", { name: "JSON" });
  expect(link).toHaveAttribute("href", href);
  expect(link).toHaveAttribute("target", "_blank");
  expect(link).toHaveAttribute("rel", "noopener noreferrer");
  expect(link).toHaveAccessibleDescription("Opens in a new tab");
  expect(link).not.toHaveAttribute("aria-disabled");
  expect(link.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  expect(screen.queryByRole("menuitem", { name: "Open on Reddit" })).not.toBeInTheDocument();
  expect(screen.queryByText(/^Fetched /)).toBeNull();
  expect(screen.queryByRole("separator")).toBeNull();

  await user.keyboard("{Escape}");

  expect(screen.queryByRole("menu")).toBeNull();
  await waitFor(() => expect(trigger).toHaveFocus());
});

test("When a menu has response age then its footer should describe the menu without becoming a keyboard stop", async () => {
  const user = userEvent.setup();
  const now = Date.now();
  const { rerender } = render(
    <PaneMenu
      jsonHref="/api/posts/example"
      redditHref="https://reddit.com"
      fetchedAt={now - 51 * 60_000}
      label="Post actions"
    />,
  );
  const trigger = screen.getByRole("button", { name: "Post actions" });

  await user.tab();
  await user.keyboard("{ArrowDown}");

  const menu = await screen.findByRole("menu");
  const age = `Fetched ${RELATIVE_FORMAT.format(-51, "minute")}`;
  const footer = screen.getByText(age);
  expect(menu).toHaveAccessibleDescription(age);
  expect(menu).not.toContainElement(footer);
  expect(footer).not.toHaveAttribute("tabindex");
  expect(screen.getByRole("separator")).toBeVisible();
  expect(screen.getAllByRole("menuitem")).toHaveLength(2);
  await user.tab();
  await waitFor(() =>
    expect(screen.getByRole("menuitem", { name: "Open on Reddit" })).toHaveFocus(),
  );

  await user.keyboard("{ArrowDown}");

  expect(screen.getByRole("menuitem", { name: "JSON" })).toHaveFocus();

  await user.keyboard("{ArrowDown}");

  expect(screen.getByRole("menuitem", { name: "Open on Reddit" })).toHaveFocus();

  rerender(
    <PaneMenu jsonHref="/api/posts/example" redditHref="https://reddit.com" label="Post actions" />,
  );

  expect(menu).not.toHaveAttribute("aria-describedby");
  expect(screen.queryByText(/^Fetched /)).toBeNull();
  expect(screen.queryByRole("separator")).toBeNull();

  await user.keyboard("{Escape}");

  await waitFor(() => expect(trigger).toHaveFocus());
});

test("When a response ages while its menu is open or closed then the footer should show the current age", async () => {
  vi.useFakeTimers({ toFake: ["Date", "setInterval", "clearInterval"] });
  onTestFinished(() => {
    vi.useRealTimers();
  });
  const user = userEvent.setup();
  const now = Date.now();
  render(<PaneMenu jsonHref="/api/posts/example" fetchedAt={now} label="Post actions" />);
  const trigger = screen.getByRole("button", { name: "Post actions" });

  await user.click(trigger);

  expect(screen.getByText("Fetched just now")).toBeVisible();

  act(() => vi.advanceTimersByTime(60_000));

  expect(screen.getByText(`Fetched ${RELATIVE_FORMAT.format(-1, "minute")}`)).toBeVisible();

  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  act(() => vi.advanceTimersByTime(60_000));
  await user.click(trigger);

  expect(screen.getByText(`Fetched ${RELATIVE_FORMAT.format(-2, "minute")}`)).toBeVisible();
});

test("When JSON is unavailable then the Reddit item should remain a keyboard-accessible new-tab link", async () => {
  const user = userEvent.setup();
  const href = "https://www.reddit.com/r/rust/hot/";
  render(<PaneMenu jsonHref={undefined} redditHref={href} label="Listing actions" />);
  const trigger = screen.getByRole("button", { name: "Listing actions" });

  await user.tab();
  await user.keyboard("{ArrowDown}");

  const link = await screen.findByRole("menuitem", { name: "Open on Reddit" });
  expect(screen.getAllByRole("menuitem")).toEqual([
    link,
    screen.getByRole("menuitem", { name: "JSON" }),
  ]);
  expect(link).toHaveAttribute("href", href);
  expect(link).toHaveAttribute("target", "_blank");
  expect(link).toHaveAttribute("rel", "noopener noreferrer");
  expect(link).toHaveAccessibleDescription("Opens in a new tab");
  expect(link).not.toHaveAttribute("aria-disabled");
  expect(link.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  expect(screen.getByRole("menuitem", { name: "JSON" })).toHaveAttribute("aria-disabled", "true");

  await user.tab();

  await waitFor(() => expect(link).toHaveFocus());

  await user.keyboard("{Escape}");

  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  await waitFor(() => expect(trigger).toHaveFocus());
});

test("When an open menu loses its JSON URL then its item should disable until a current URL is available", async () => {
  const user = userEvent.setup();
  const first = new URL("/api/posts/example/comments?sort=best", location.origin).href;
  const next = new URL("/api/posts/example/comments?sort=new", location.origin).href;
  const { rerender } = render(<PaneMenu jsonHref={first} label="Comments actions" />);

  await user.click(screen.getByRole("button", { name: "Comments actions" }));

  expect(await screen.findByRole("menuitem", { name: "JSON" })).toHaveAttribute("href", first);

  rerender(<PaneMenu jsonHref={undefined} label="Comments actions" />);

  const link = screen.getByRole("menuitem", { name: "JSON" });
  expect(link).toBeVisible();
  expect(link).toHaveAttribute("aria-disabled", "true");
  expect(link).not.toHaveAttribute("href");

  await user.click(link);

  expect(screen.getByRole("menu")).toBeVisible();

  rerender(<PaneMenu jsonHref={next} label="Comments actions" />);

  expect(screen.getByRole("menuitem", { name: "JSON" })).toHaveAttribute("href", next);
  expect(screen.getByRole("menuitem", { name: "JSON" })).not.toHaveAttribute("aria-disabled");
});
