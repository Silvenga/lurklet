import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { HomePage } from "./HomePage";

test("When the home page renders then it should show the application name", () => {
  render(<HomePage />);

  expect(screen.getByRole("heading", { level: 1, name: "Lurklet" })).toBeInTheDocument();
});
