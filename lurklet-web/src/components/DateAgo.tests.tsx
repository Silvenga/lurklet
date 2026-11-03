import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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

test("When the date is hovered then its tooltip should stay closed regardless of elapsed time", async () => {
  vi.useFakeTimers();
  const dateTime = "2026-10-26T11:00:00Z";
  render(<DateAgo dateTime={dateTime} />);
  const trigger = screen.getByRole("button", { name: RELATIVE_FORMAT.format(-1, "hour") });

  fireEvent.pointerMove(document.body, { pointerType: "mouse" });
  fireEvent.pointerOver(trigger, { pointerType: "mouse" });

  expect(screen.queryByRole("tooltip")).toBeNull();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(5000);
  });

  expect(screen.queryByRole("tooltip")).toBeNull();
});

test.each(["click", "touch", "Enter", "Space"])(
  "When the date receives %s activation then its tooltip should toggle immediately and reopen after Escape",
  async (activation) => {
    const user = userEvent.setup();
    const dateTime = "2026-10-26T11:00:00Z";
    const fullDate = DATE_FORMAT.format(new Date(dateTime));
    render(<DateAgo dateTime={dateTime} />);
    const trigger = screen.getByRole("button", { name: RELATIVE_FORMAT.format(-1, "hour") });

    if (activation === "Enter" || activation === "Space") {
      await user.tab();
      await user.keyboard("{Escape}");
    }

    const activate = async () => {
      if (activation === "click") {
        await user.click(trigger);
      } else if (activation === "touch") {
        await user.pointer({ keys: "[TouchA]", target: trigger });
      } else {
        await user.keyboard(activation === "Enter" ? "{Enter}" : " ");
      }
    };

    await activate();

    expect(screen.getByRole("tooltip")).toHaveTextContent(fullDate);
    expect(trigger).toHaveAccessibleDescription(fullDate);

    await activate();

    expect(screen.queryByRole("tooltip")).toBeNull();

    await activate();

    expect(screen.getByRole("tooltip")).toHaveTextContent(fullDate);

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("tooltip")).toBeNull();

    await activate();

    expect(screen.getByRole("tooltip")).toHaveTextContent(fullDate);
  },
);

test("When focus leaves a date while its tooltip is hovered then the tooltip should close", async () => {
  const user = userEvent.setup();
  render(
    <>
      <DateAgo dateTime="2026-10-26T11:00:00Z" />
      <Button>Next control</Button>
    </>,
  );
  await user.tab();
  await user.hover(screen.getByRole("tooltip"));

  await user.tab();

  expect(screen.getByRole("button", { name: "Next control" })).toHaveFocus();
  expect(screen.queryByRole("tooltip")).toBeNull();
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
