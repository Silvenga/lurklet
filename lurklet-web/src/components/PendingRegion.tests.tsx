import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { PendingRegion } from "./PendingRegion";

test("When a region becomes pending then its content and local state should stay mounted", () => {
  const { container, rerender } = render(
    <PendingRegion pending={false} className="flex">
      <input aria-label="Example" defaultValue="Initial" />
    </PendingRegion>,
  );
  const input = screen.getByRole("textbox", { name: "Example" });
  fireEvent.change(input, { target: { value: "Edited" } });

  rerender(
    <PendingRegion pending className="flex">
      <input aria-label="Example" defaultValue="Initial" />
    </PendingRegion>,
  );

  expect(screen.getByRole("textbox", { name: "Example" })).toBe(input);
  expect(input).toHaveValue("Edited");
  expect(input).toBeVisible();
  expect(container.firstElementChild).toHaveAttribute("aria-busy", "true");
  expect(container.firstElementChild).toHaveClass("flex", "opacity-80");

  rerender(
    <PendingRegion pending={false} className="flex">
      <input aria-label="Example" defaultValue="Initial" />
    </PendingRegion>,
  );

  expect(screen.getByRole("textbox", { name: "Example" })).toBe(input);
  expect(container.firstElementChild).toHaveAttribute("aria-busy", "false");
  expect(container.firstElementChild).not.toHaveClass("opacity-80");
});
