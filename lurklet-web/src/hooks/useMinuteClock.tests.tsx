import { act, cleanup, renderHook } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useMinuteClock } from "./useMinuteClock";

const NOW = Date.parse("2026-10-26T12:00:00Z");

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

test("When multiple dates subscribe then one clock should update them without network requests", async () => {
  const fetch = vi.spyOn(globalThis, "fetch");
  const first = renderHook(() => useMinuteClock());
  const second = renderHook(() => useMinuteClock());
  expect(first.result.current).toBe(NOW);
  expect(second.result.current).toBe(NOW);
  expect(vi.getTimerCount()).toBe(1);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });

  expect(first.result.current).toBe(NOW + 60_000);
  expect(second.result.current).toBe(NOW + 60_000);
  expect(fetch).not.toHaveBeenCalled();

  first.unmount();

  expect(vi.getTimerCount()).toBe(1);

  second.unmount();

  expect(vi.getTimerCount()).toBe(0);
});

test("When a date mounts after the clock was idle then it should use the current time", async () => {
  const first = renderHook(() => useMinuteClock());
  first.unmount();
  await vi.advanceTimersByTimeAsync(30 * 60_000);

  const next = renderHook(() => useMinuteClock());

  expect(next.result.current).toBe(NOW + 30 * 60_000);
  expect(vi.getTimerCount()).toBe(1);
});

test("When Strict Mode repeats subscription setup then only one timer should remain active", () => {
  const { unmount } = renderHook(() => [useMinuteClock(), useMinuteClock()], {
    wrapper: ({ children }) => <StrictMode>{children}</StrictMode>,
  });

  expect(vi.getTimerCount()).toBe(1);

  unmount();

  expect(vi.getTimerCount()).toBe(0);
});
