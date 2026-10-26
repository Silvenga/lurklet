import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import { DateAgo } from "./DateAgo";

const NOW = Date.parse("2026-10-26T12:00:00Z");
const RELATIVE_FORMAT = new Intl.RelativeTimeFormat(undefined, { numeric: "always" });
const DATE_FORMAT = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "long" });

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

test.each([
  [0, 0, null],
  [59, 0, null],
  [60, -1, "minute"],
  [119, -1, "minute"],
  [120, -2, "minute"],
  [3599, -59, "minute"],
  [3600, -1, "hour"],
  [7199, -1, "hour"],
  [86399, -23, "hour"],
  [86400, -1, "day"],
  [2591999, -29, "day"],
  [2592000, -1, "month"],
  [31536000, -1, "year"],
  [63072000, -2, "year"],
  [-59, 0, null],
  [-60, 1, "minute"],
  [-3600, 1, "hour"],
  [-86400, 1, "day"],
  [-2592000, 1, "month"],
  [-31536000, 1, "year"],
] as const)(
  "When a timestamp is %i seconds old then DateAgo should show the expected relative unit",
  (secondsAgo, value, unit) => {
    const dateTime = new Date(NOW - secondsAgo * 1000).toISOString();
    const label = unit === null ? "just now" : RELATIVE_FORMAT.format(value, unit);

    const { container } = render(<DateAgo dateTime={dateTime} className="align-baseline" />);

    const trigger = screen.getByRole("button", { name: label });
    expect(trigger).toHaveTextContent(label);
    expect(trigger).not.toHaveAttribute("aria-label");
    expect(trigger).toHaveClass("align-baseline");
    expect(trigger).toHaveClass("font-[weight:inherit]");
    expect(trigger).not.toHaveClass("font-medium");
    expect(trigger.querySelector("time")).toHaveAttribute("datetime", dateTime);
    expect(trigger.querySelector("time")).toHaveTextContent(label);
    expect(trigger.querySelector("time")).not.toHaveAttribute("aria-hidden");
    expect(container.querySelector("[aria-live], .sr-only, [title]")).toBeNull();
    expect(screen.queryByRole("tooltip")).toBeNull();
  },
);

test("When the date receives keyboard focus then its tooltip should describe the exact local timestamp", async () => {
  const user = userEvent.setup();
  const dateTime = "2026-10-26T07:00:00-04:00";
  const fullDate = DATE_FORMAT.format(new Date(dateTime));
  render(<DateAgo dateTime={dateTime} />);
  const trigger = screen.getByRole("button", { name: RELATIVE_FORMAT.format(-1, "hour") });

  await user.tab();

  expect(trigger).toHaveFocus();
  const tooltip = await screen.findByRole("tooltip");
  expect(tooltip).toHaveTextContent(fullDate);
  expect(trigger).toHaveAttribute("aria-describedby", tooltip.id);
  expect(trigger).toHaveAccessibleDescription(fullDate);
  expect(trigger).toHaveAccessibleName(RELATIVE_FORMAT.format(-1, "hour"));
  expect(trigger.querySelector("time")).toHaveAttribute("datetime", dateTime);

  await user.keyboard("{Escape}");

  await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
  expect(trigger).toHaveFocus();
});

test("When the date is hovered then the same full timestamp should be visible", async () => {
  const user = userEvent.setup();
  const dateTime = "2026-10-26T11:00:00Z";
  render(<DateAgo dateTime={dateTime} />);

  await user.hover(document.body);
  await user.hover(screen.getByRole("button", { name: RELATIVE_FORMAT.format(-1, "hour") }));

  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    DATE_FORMAT.format(new Date(dateTime)),
  );
});

test("When a date is activated inside a comment disclosure then it should not collapse the comment", async () => {
  const user = userEvent.setup();
  render(
    <Collapsible defaultExpanded>
      <Button slot="trigger">Toggle comment</Button>
      <DateAgo dateTime="2026-10-26T11:00:00Z" />
      <CollapsibleContent>Comment body</CollapsibleContent>
    </Collapsible>,
  );

  await user.click(screen.getByRole("button", { name: RELATIVE_FORMAT.format(-1, "hour") }));

  expect(screen.getByRole("button", { name: "Toggle comment" })).toHaveAttribute(
    "aria-expanded",
    "true",
  );
  expect(screen.getByText("Comment body")).toBeVisible();
});

test("When a date is activated inside a form then it should not submit the form", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(
    <form
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <DateAgo dateTime="2026-10-26T11:00:00Z" />
    </form>,
  );

  await user.click(screen.getByRole("button", { name: RELATIVE_FORMAT.format(-1, "hour") }));

  expect(onSubmit).not.toHaveBeenCalled();
});

test("When the shared minute clock ticks then all displayed relative labels should update", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
  const { container } = render(
    <>
      <DateAgo dateTime="2026-10-26T11:59:00Z" />
      <DateAgo dateTime="2026-10-26T11:58:00Z" />
    </>,
  );

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });

  expect(screen.getByRole("button", { name: RELATIVE_FORMAT.format(-2, "minute") })).toBeVisible();
  expect(screen.getByRole("button", { name: RELATIVE_FORMAT.format(-3, "minute") })).toBeVisible();
  expect(container.querySelector("[aria-live]")).toBeNull();
});
