import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { PaneMenu } from "./PaneMenu";

test("When a pane menu opens with the keyboard then its JSON item should be a new-tab link and Escape should restore focus", async () => {
  const user = userEvent.setup();
  const href = new URL("/api/posts/example", location.origin).href;
  render(<PaneMenu jsonHref={href} label="Post actions" />);
  const trigger = screen.getByRole("button", { name: "Post actions" });

  await user.tab();

  expect(trigger).toHaveFocus();
  expect(trigger).toHaveAttribute("data-variant", "ghost");
  expect(trigger).toHaveClass("absolute", "top-0", "right-0");
  expect(screen.queryByRole("menuitem")).toBeNull();

  await user.keyboard("{ArrowDown}");

  const link = await screen.findByRole("menuitem", { name: "JSON" });
  expect(link.tagName).toBe("A");
  expect(link).toHaveAttribute("href", href);
  expect(link).toHaveAttribute("target", "_blank");
  expect(link).toHaveAttribute("rel", "noopener noreferrer");
  expect(link).toHaveAccessibleDescription("Opens in a new tab");
  expect(link).not.toHaveAttribute("aria-disabled");
  expect(link.querySelector("svg")).toHaveAttribute("aria-hidden", "true");

  await user.keyboard("{Escape}");

  expect(screen.queryByRole("menu")).toBeNull();
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
