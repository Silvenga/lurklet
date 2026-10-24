import { expect, test } from "vitest";
import { cn } from "./cn";

test("When classes conflict then the last utility should take precedence", () => {
  const result = cn("px-4 py-2", "px-6");

  expect(result).toBe("py-2 px-6");
});

test("When conditional classes are absent then they should be ignored", () => {
  const result = cn("text-sm", false, null, undefined, ["font-medium", ["px-4"]]);

  expect(result).toBe("text-sm font-medium px-4");
});

test("When variants differ then their utilities should remain independent", () => {
  const result = cn("bg-background dark:bg-muted", "dark:bg-card");

  expect(result).toBe("bg-background dark:bg-card");
});
