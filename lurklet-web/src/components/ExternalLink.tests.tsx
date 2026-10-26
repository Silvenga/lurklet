import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { expect, test, vi } from "vitest";
import { ExternalLink } from "./ExternalLink";

test("When an external link renders then it should forward anchor props with the external link policy", () => {
  const ref = createRef<HTMLAnchorElement>();

  render(
    <ExternalLink
      href="https://example.com/article"
      className="underline"
      aria-label="Read article"
      ref={ref}
    >
      Article
    </ExternalLink>,
  );

  const link = screen.getByRole("link", { name: "Read article" });
  expect(link.tagName).toBe("A");
  expect(link).toHaveTextContent("Article");
  expect(link).toHaveClass("underline");
  expect(link).toHaveAttribute("href", "https://example.com/article");
  expect(link).toHaveAttribute("target", "_blank");
  expect(link).toHaveAttribute("rel", "noopener noreferrer nofollow ugc external");
  expect(ref.current).toBe(link);
  const icon = link.querySelector("svg");
  expect(icon).toBe(link.lastElementChild);
  expect(icon).toHaveAttribute("aria-hidden", "true");
  expect(icon).toHaveAttribute("focusable", "false");
  expect(icon).toHaveClass("size-[1em]", "align-middle");
});

test("When an external link is clicked then its supplied click handler should run", async () => {
  const user = userEvent.setup();
  const onClick = vi.fn();
  render(
    <ExternalLink
      href="https://example.com/article"
      onClick={(event) => {
        event.preventDefault();
        onClick();
      }}
    >
      Article
    </ExternalLink>,
  );

  await user.click(screen.getByRole("link", { name: "Article" }));

  expect(onClick).toHaveBeenCalledTimes(1);
});
