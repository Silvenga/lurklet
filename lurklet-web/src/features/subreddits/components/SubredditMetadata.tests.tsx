import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { expect, onTestFinished, test } from "vitest";
import { SUBREDDIT_RESPONSE } from "@/_tests/mocks/fixtures";
import { createQueryClient } from "@/app/query-client";
import { getSubredditQueryKey } from "@/clients/lurklet-api/generated/@tanstack/react-query.gen";
import type { Subreddit } from "@/clients/lurklet-api/generated/types.gen";
import { SubredditMetadata } from "./SubredditMetadata";

function renderMetadata(value: Subreddit) {
  const queryClient = createQueryClient();
  queryClient.setQueryData(getSubredditQueryKey({ path: { subreddit: "rust" } }), value);
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

test("When metadata is available then it should show the summary and accurately labeled counts", () => {
  const { container } = renderMetadata(SUBREDDIT_RESPONSE);

  expect(screen.getByText(SUBREDDIT_RESPONSE.title, { selector: "p" })).toBeVisible();
  expect(screen.getByText(SUBREDDIT_RESPONSE.shortDescription)).toBeVisible();
  expect(screen.getByText(SUBREDDIT_RESPONSE.shortDescription)).toHaveClass(
    "prose",
    "prose-sm",
    "dark:prose-invert",
  );
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
  for (const label of ["Subscribers", "Weekly active users", "Weekly contributions", "Created"]) {
    expect(screen.getByText(label)).toHaveClass("whitespace-nowrap");
  }
  expect([...container.querySelectorAll("dt")].map((label) => label.textContent)).toEqual([
    "Subscribers",
    "Created",
    "Weekly active users",
    "Weekly contributions",
  ]);
  expect(screen.getByText("Subscribers").parentElement).not.toHaveClass("col-span-2");
  expect(screen.getByText("Created").parentElement).not.toHaveClass("col-span-2");
  expect(screen.getByText("Weekly active users").parentElement).toHaveClass("col-span-2");
  expect(screen.getByText("Weekly contributions").parentElement).toHaveClass("col-span-2");
  expect(screen.queryByRole("link", { name: "The Rust Community Code of Conduct" })).toBeNull();
  expect(screen.queryByRole("heading", { name: "Observe our code of conduct" })).toBeNull();
  expect(container.querySelectorAll(".prose")).toHaveLength(1);
  expect(container.firstElementChild).toHaveClass("select-text");
  expect(screen.queryByRole("img")).toBeNull();
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
  });

  expect(container.textContent).toBe("");
  expect(container.querySelector("dl, time, .prose")).toBeNull();
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
    interstitialWarningMarkdown: "Please **read this notice**.",
    quarantineMessageMarkdown: "Review the [community warning](https://example.com/warning).",
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
    banMessageMarkdown: "Access requires **approval**.",
    forbiddenReason: "restricted",
    interstitialWarningMarkdown: "Contact the moderators.",
    quarantineMessageMarkdown: "Additional notice.",
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
