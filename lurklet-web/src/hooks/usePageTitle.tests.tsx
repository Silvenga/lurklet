import { renderHook } from "@testing-library/react";
import { expect, test } from "vitest";
import { usePageTitle } from "./usePageTitle";

test("When the page unmounts then its title should reset to the application fallback", () => {
  const { unmount } = renderHook(() => usePageTitle("r/rust"));

  unmount();

  expect(document.title).toBe("Lurklet");
});
