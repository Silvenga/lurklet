import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect } from "vitest";

export async function expectRedditMenu(label: string, href: string | undefined) {
  const user = userEvent.setup();
  const trigger = screen.getByRole("button", { name: label });
  await user.click(trigger);
  await screen.findByRole("menu");

  if (href) {
    expect(screen.getByRole("menuitem", { name: "Open on Reddit" })).toHaveAttribute("href", href);
  } else {
    expect(screen.queryByRole("menuitem", { name: "Open on Reddit" })).not.toBeInTheDocument();
  }

  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  await waitFor(() => expect(trigger).toHaveFocus());
}
