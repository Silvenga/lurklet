import { renderHook } from "@testing-library/react";
import { StrictMode } from "react";
import { expect, test } from "vitest";
import { usePageTitle } from "./usePageTitle";

test("When the page title changes then the document title should use the current title and suffix", () => {
  const { rerender } = renderHook(({ title }) => usePageTitle(title), {
    initialProps: { title: "Home" },
    wrapper: StrictMode,
  });
  expect(document.title).toBe("Home - Lurklet");

  rerender({ title: 'Rust & <React> "discussion"' });

  expect(document.title).toBe('Rust & <React> "discussion" - Lurklet');
  expect(document.head.querySelector("react")).toBeNull();
});

test("When the page unmounts then its title should reset to the application fallback", () => {
  const { unmount } = renderHook(() => usePageTitle("r/rust"));

  unmount();

  expect(document.title).toBe("Lurklet");
});
