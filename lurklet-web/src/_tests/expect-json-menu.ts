import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect } from "vitest";

export async function expectJsonMenu(
  label: string,
  href: string | undefined,
  user = userEvent.setup(),
) {
  const trigger = await screen.findByRole("button", { name: label });
  await user.click(trigger);
  const item = await screen.findByRole("menuitem", { name: "JSON" });

  if (href) {
    expect(item).toHaveAttribute("href", href);
    expect(item).not.toHaveAttribute("aria-disabled");
  } else {
    expect(item).not.toHaveAttribute("href");
    expect(item).toHaveAttribute("aria-disabled", "true");
  }

  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
}
