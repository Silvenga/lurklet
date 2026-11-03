import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { POST_RESPONSE, SUBREDDIT_PAGE_RESPONSE } from "@/_tests/mocks/fixtures";
import type { PostFlair as Flair } from "@/clients/lurklet-api/generated/types.gen";
import { PostFlair } from "./PostFlair";

test.each([
  [POST_RESPONSE.flair, "#000000"],
  [SUBREDDIT_PAGE_RESPONSE.posts[2]!.flair!, "#ffffff"],
] as const)(
  "When flair is %j then its text and subreddit colors should be preserved",
  (flair, color) => {
    render(<PostFlair flair={flair} />);

    const label = screen.getByText(flair.text);
    expect(label).toBeVisible();
    expect(label.closest('[data-slot="badge"]')).toHaveStyle({
      "background-color": flair.backgroundColor,
      color,
    });
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  },
);

test("When flair is absent then no badge should be rendered", () => {
  const { container } = render(<PostFlair flair={null} />);

  expect(container).toBeEmptyDOMElement();
});

test.each(["url(https://example.com/track)", "var(--external)", "red", "#123456; color: red"])(
  "When a flair color is %s then it should not be applied as CSS",
  (backgroundColor) => {
    const flair: Flair = {
      ...POST_RESPONSE.flair,
      text: "<b>Untrusted</b>",
      backgroundColor,
      textColor: null,
    };

    const { container } = render(<PostFlair flair={flair} />);

    const label = screen.getByText("<b>Untrusted</b>");
    expect(label.closest('[data-slot="badge"]')).not.toHaveAttribute("style");
    expect(container.querySelector("b")).toBeNull();
  },
);

test("When a flair emoji fails then its alias should replace it without losing surrounding text", () => {
  const flair = SUBREDDIT_PAGE_RESPONSE.posts[3]!.flair!;
  const { container } = render(<PostFlair flair={flair} />);
  const image = screen.getByRole("img", { name: ":post-daily-discussion:" });
  const url = new URL(image.getAttribute("src")!);
  expect(url.origin).toBe(location.origin);
  expect(url.pathname).toBe("/api/media");
  expect(url.searchParams.get("url")).toBe(
    "https://emoji.redditmedia.com/fixture_t5_fixture/post-daily-discussion",
  );
  expect(image).toHaveAttribute("referrerpolicy", "no-referrer");
  expect(image.parentElement?.textContent).toBe(" Daily Discussion");

  fireEvent.error(image);

  const fallback = screen.getByText(":post-daily-discussion:");
  expect(fallback.parentElement?.textContent).toBe(flair.text);
  expect(container.querySelector("img, [src], [srcset], [href]")).toBeNull();
});
