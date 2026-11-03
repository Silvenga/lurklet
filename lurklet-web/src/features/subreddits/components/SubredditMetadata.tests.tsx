import { QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { expect, onTestFinished, test } from "vitest";
import { expectJsonMenu } from "@/_tests/expect-json-menu";
import { SUBREDDIT_RESPONSE } from "@/_tests/mocks/fixtures";
import { createQueryClient } from "@/app/query-client";
import { getSubredditQueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type { Subreddit } from "@/clients/lurklet-api/generated/types.gen";
import { SubredditMetadata } from "./SubredditMetadata";

function renderMetadata(value: Subreddit) {
  const queryClient = createQueryClient();
  queryClient.setQueryData(getSubredditQueryKey({ path: { subreddit: "rust" } }), {
    data: value,
    fetchedAt: undefined,
  });
  const result = render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <SubredditMetadata subreddit="rust" />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  onTestFinished(() => {
    result.unmount();
    queryClient.clear();
  });
  return result;
}

test("When metadata is available then it should show the summary and accurately labeled counts", async () => {
  const { container } = renderMetadata(SUBREDDIT_RESPONSE);

  expect(screen.getByText(SUBREDDIT_RESPONSE.title, { selector: "p" })).toBeVisible();
  expect(screen.getByText(SUBREDDIT_RESPONSE.shortDescription)).toBeVisible();
  expect(screen.getByText("Subscribers").nextElementSibling).toHaveTextContent(
    new Intl.NumberFormat().format(SUBREDDIT_RESPONSE.subscriberCount),
  );
  expect(screen.getByText("Weekly active users").nextElementSibling).toHaveTextContent(
    new Intl.NumberFormat().format(SUBREDDIT_RESPONSE.weeklyActiveUserCount),
  );
  expect(screen.getByText("Weekly contributions").nextElementSibling).toHaveTextContent(
    new Intl.NumberFormat().format(SUBREDDIT_RESPONSE.weeklyContributionCount),
  );
  expect(screen.queryByText(/currently online|users online/i)).toBeNull();
  expect(screen.queryByText("Community type")).toBeNull();
  expect(screen.queryByText("Public")).toBeNull();
  expect(screen.queryByText("Posting restricted")).toBeNull();
  expect(screen.queryByText("NSFW")).toBeNull();
  expect(screen.queryByText("Quarantined")).toBeNull();
  expect(container.querySelector("time")).toHaveAttribute("datetime", SUBREDDIT_RESPONSE.createdAt);
  expect(screen.getByText("Created").tagName).toBe("DT");
  expect(screen.getByText("Created").nextElementSibling?.tagName).toBe("DD");
  expect(screen.getByText("Created").nextElementSibling).toContainElement(
    container.querySelector("time"),
  );
  expect(screen.queryByRole("link", { name: "The Rust Community Code of Conduct" })).toBeNull();
  expect(screen.queryByRole("heading", { name: "Observe our code of conduct" })).toBeNull();
  const icon = screen.getByRole("img", { name: "r/rust icon" });
  const iconUrl = new URL(icon.getAttribute("src")!);
  expect(iconUrl.origin).toBe(location.origin);
  expect(iconUrl.pathname).toBe("/api/media");
  expect(iconUrl.searchParams.get("url")).toBe(SUBREDDIT_RESPONSE.styles.iconUrl);
  expect(screen.getByRole("link", { name: "r/rust" })).toHaveAttribute("href", "/r/rust");
  await expectJsonMenu("Subreddit actions", new URL("/api/subreddits/rust", location.origin).href);
});

test("When metadata fields are unknown then they should be omitted rather than inferred", () => {
  const { container } = renderMetadata({
    ...SUBREDDIT_RESPONSE,
    title: null,
    shortDescription: null,
    descriptionMarkdown: null,
    subscriberCount: null,
    weeklyActiveUserCount: null,
    weeklyContributionCount: null,
    communityType: null,
    createdAt: null,
    isNsfw: null,
    isQuarantined: null,
    isPostingRestricted: null,
    styles: null,
  });

  expect(container.textContent).toBe("r/rust");
  expect(container.querySelector("dl, time, .prose")).toBeNull();
  expect(screen.queryByRole("img")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("When counts are zero then metadata should display them", () => {
  renderMetadata({
    ...SUBREDDIT_RESPONSE,
    subscriberCount: 0,
    weeklyActiveUserCount: 0,
    weeklyContributionCount: 0,
  });

  expect(screen.getByText("Subscribers").nextElementSibling).toHaveTextContent(/^0$/);
  expect(screen.getByText("Weekly active users").nextElementSibling).toHaveTextContent(/^0$/);
  expect(screen.getByText("Weekly contributions").nextElementSibling).toHaveTextContent(/^0$/);
});

test("When only the creation date is known then it should still appear as a label and value", () => {
  const { container } = renderMetadata({
    ...SUBREDDIT_RESPONSE,
    communityType: null,
    subscriberCount: null,
    weeklyActiveUserCount: null,
    weeklyContributionCount: null,
  });

  expect(container.querySelectorAll("dt")).toHaveLength(1);
  const label = screen.getByText("Created");
  expect(label.tagName).toBe("DT");
  expect(label.nextElementSibling?.tagName).toBe("DD");
  expect(label.nextElementSibling?.querySelector("time")).toHaveAttribute(
    "datetime",
    SUBREDDIT_RESPONSE.createdAt,
  );
});

test("When an available community has warnings then its flags and Markdown notices should appear", () => {
  renderMetadata({
    ...SUBREDDIT_RESPONSE,
    isNsfw: true,
    isQuarantined: true,
    interstitialWarningMarkdown: { content: "Please **read this notice**.", media: [] },
    quarantineMessageMarkdown: {
      content: "Review the [community warning](https://example.com/warning).",
      media: [],
    },
  });

  expect(screen.getByText("NSFW")).toBeVisible();
  expect(screen.getByText("Quarantined")).toBeVisible();
  expect(screen.getByText("Community notice")).toBeVisible();
  expect(screen.getByText("read this notice").tagName).toBe("STRONG");
  expect(screen.getByText("Quarantine notice")).toBeVisible();
  expect(screen.getByRole("link", { name: "community warning" })).toHaveAttribute(
    "href",
    "https://example.com/warning",
  );
});

test("When a community is unavailable then the provided explanation should render without inventing metadata", () => {
  const { container } = renderMetadata({
    kind: "unavailable",
    id: null,
    banTitle: "Community access is limited",
    banMessageMarkdown: { content: "Access requires **approval**.", media: [] },
    forbiddenReason: "restricted",
    interstitialWarningMarkdown: { content: "Contact the moderators.", media: [] },
    quarantineMessageMarkdown: { content: "Additional notice.", media: [] },
  });

  expect(screen.getByText("Community access is limited")).toBeVisible();
  expect(screen.getByText("approval").tagName).toBe("STRONG");
  expect(screen.getByText("restricted")).toBeVisible();
  expect(screen.getByText("Contact the moderators.")).toBeVisible();
  expect(screen.getByText("Additional notice.")).toBeVisible();
  expect(screen.queryByText("Subscribers")).toBeNull();
  expect(container.querySelector("time")).toBeNull();
});

test("When an unavailable community has no explanation then it should show a neutral fallback", () => {
  renderMetadata({
    kind: "unavailable",
    id: null,
    banTitle: null,
    banMessageMarkdown: null,
    forbiddenReason: null,
    interstitialWarningMarkdown: null,
    quarantineMessageMarkdown: null,
  });

  expect(screen.getByRole("alert")).toHaveTextContent("Subreddit unavailable");
  expect(screen.getByText("This subreddit is unavailable.")).toBeVisible();
  expect(screen.queryByText(/banned|private|deleted/i)).toBeNull();
});

test.each([null, ""])(
  "When the primary subreddit icon is %j then the legacy icon should be proxied",
  (iconUrl) => {
    renderMetadata({
      ...SUBREDDIT_RESPONSE,
      styles: { ...SUBREDDIT_RESPONSE.styles, iconUrl },
    });

    const icon = screen.getByRole("img", { name: "r/rust icon" });

    expect(new URL(icon.getAttribute("src")!).searchParams.get("url")).toBe(
      SUBREDDIT_RESPONSE.styles.legacyIconUrl,
    );
  },
);

test("When neither subreddit icon is available then the heading should render without an image", () => {
  renderMetadata({
    ...SUBREDDIT_RESPONSE,
    styles: { ...SUBREDDIT_RESPONSE.styles, iconUrl: null, legacyIconUrl: null },
  });

  expect(screen.getByRole("heading", { name: "r/rust" })).toBeVisible();
  expect(screen.getByText(SUBREDDIT_RESPONSE.title)).toBeVisible();
  expect(screen.queryByRole("img")).toBeNull();
});

test("When the subreddit icon fails then its placeholder should not hide metadata or request an upstream fallback", () => {
  const { container } = renderMetadata(SUBREDDIT_RESPONSE);

  fireEvent.error(screen.getByRole("img", { name: "r/rust icon" }));

  expect(screen.getByRole("img", { name: "Image unavailable: r/rust icon" })).toBeVisible();
  expect(screen.getByRole("link", { name: "r/rust" })).toBeVisible();
  expect(screen.getByText(SUBREDDIT_RESPONSE.title)).toBeVisible();
  expect(container.querySelector("img, [src], [srcset]")).toBeNull();
});
