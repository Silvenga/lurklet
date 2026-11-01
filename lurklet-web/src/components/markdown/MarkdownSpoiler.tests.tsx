import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { MarkdownSpoiler } from "./MarkdownSpoiler";

test("When a spoiler is revealed with the keyboard then its content should become accessible without nesting links in the button", async () => {
  const user = userEvent.setup();
  const { container } = render(
    <MarkdownSpoiler>
      <strong>Secret</strong> <a href="/r/rust">Rust</a>
    </MarkdownSpoiler>,
  );
  const button = screen.getByRole("button", { name: "Show spoiler", expanded: false });
  const content = screen.getByText("Secret").parentElement!;
  expect(content).toHaveAttribute("aria-hidden", "true");
  expect(content).toHaveAttribute("inert");
  expect(content).toHaveClass("invisible");
  expect(screen.queryByRole("link")).toBeNull();

  await user.tab();
  await user.keyboard("{Enter}");

  expect(button).toHaveFocus();
  expect(button).toHaveAccessibleName("Hide spoiler");
  expect(button).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByText("Secret")).toBeVisible();
  expect(content).toHaveAttribute("aria-hidden", "false");
  expect(content).not.toHaveAttribute("inert");
  expect(content).not.toHaveClass("invisible");
  expect(screen.getByRole("link", { name: "Rust" })).toBeVisible();
  expect(container.querySelector("button a, button button")).toBeNull();

  await user.keyboard(" ");

  expect(button).toHaveAccessibleName("Show spoiler");
  expect(button).toHaveAttribute("aria-expanded", "false");
  expect(content).toHaveAttribute("aria-hidden", "true");
  expect(content).toHaveAttribute("inert");
  expect(content).toHaveClass("invisible");
});

test("When a spoiler mask and its revealed text are clicked then they should toggle without hiding for link clicks", async () => {
  const user = userEvent.setup();
  render(
    <MarkdownSpoiler>
      <strong>Secret</strong> <a href="#destination">Link</a>
    </MarkdownSpoiler>,
  );
  const button = screen.getByRole("button", { name: "Show spoiler" });
  const mask = button.parentElement!;
  expect(mask).toHaveClass("bg-spoiler");

  await user.click(mask);

  expect(button).toHaveAccessibleName("Hide spoiler");
  expect(mask).not.toHaveClass("bg-spoiler");
  expect(mask).toHaveClass("bg-spoiler-revealed");
  await user.click(screen.getByRole("link", { name: "Link" }));
  expect(button).toHaveAttribute("aria-expanded", "true");

  await user.click(screen.getByText("Secret"));

  expect(button).toHaveAccessibleName("Show spoiler");
  expect(mask).toHaveClass("bg-spoiler");
  expect(mask).not.toHaveClass("bg-spoiler-revealed");
  expect(screen.queryByRole("link")).toBeNull();
});

test("When nested spoilers are toggled then the containing spoiler should remain revealed", async () => {
  const user = userEvent.setup();
  render(
    <MarkdownSpoiler>
      Outer <MarkdownSpoiler>Inner</MarkdownSpoiler>
    </MarkdownSpoiler>,
  );

  await user.click(screen.getByRole("button", { name: "Show spoiler" }));
  await user.click(screen.getByRole("button", { name: "Show spoiler" }).parentElement!);

  expect(screen.getAllByRole("button", { name: "Hide spoiler" })).toHaveLength(2);

  await user.click(screen.getByText("Inner"));

  expect(screen.getAllByRole("button", { name: "Hide spoiler" })).toHaveLength(1);
  expect(screen.getAllByRole("button", { name: "Show spoiler" })).toHaveLength(1);
});
