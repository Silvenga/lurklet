import { render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";
import { Markdown } from "./Markdown";

test("When Markdown contains headings and inline formatting then it should render semantic elements", () => {
  const content = "# Introduction\n\nA **bold** and *emphasized* paragraph with `inline code`.";

  render(<Markdown>{content}</Markdown>);

  expect(screen.getByRole("heading", { level: 1, name: "Introduction" })).toBeInTheDocument();
  expect(screen.getByText("bold").tagName).toBe("STRONG");
  expect(screen.getByText("emphasized").tagName).toBe("EM");
  expect(screen.getByText("inline code").tagName).toBe("CODE");
  expect(screen.getByText(/paragraph with/).tagName).toBe("P");
});

test("When Markdown contains lists and a quotation then it should preserve their structure", () => {
  const content = "> Quoted text\n\n- First item\n- Second item\n\n1. Ordered item\n\n---";

  render(<Markdown>{content}</Markdown>);

  expect(screen.getByText("Quoted text").closest("blockquote")).toBeInTheDocument();
  expect(screen.getAllByRole("list").map((list) => list.tagName)).toEqual(["UL", "OL"]);
  expect(screen.getAllByRole("listitem")).toHaveLength(3);
  expect(screen.getByRole("separator")).toBeInTheDocument();
});

test("When Markdown uses GFM extensions then it should render tables tasks strikethrough and autolinks", () => {
  const content = [
    "| Name | Value |",
    "| --- | --- |",
    "| Example | 42 |",
    "",
    "- [x] Complete",
    "- [ ] Pending",
    "",
    "~~Removed~~",
    "",
    "https://example.com",
  ].join("\n");

  render(<Markdown>{content}</Markdown>);

  const table = screen.getByRole("table");
  expect(within(table).getByRole("columnheader", { name: "Name" })).toBeInTheDocument();
  expect(within(table).getByRole("cell", { name: "42" })).toBeInTheDocument();
  const [complete, pending] = screen.getAllByRole("checkbox");
  expect(complete).toBeChecked();
  expect(complete).toBeDisabled();
  expect(pending).not.toBeChecked();
  expect(pending).toBeDisabled();
  expect(screen.getByText("Removed").tagName).toBe("DEL");
  expect(screen.getByRole("link", { name: "https://example.com" })).toHaveAttribute(
    "href",
    "https://example.com",
  );
});

test("When Markdown contains a fenced code block then its contents should remain literal", () => {
  const content = "```html\n<strong>Not markup</strong>\n```";

  const { container } = render(<Markdown>{content}</Markdown>);

  const code = screen.getByText("<strong>Not markup</strong>");
  expect(code.tagName).toBe("CODE");
  expect(code.closest("pre")).toBeInTheDocument();
  expect(container.querySelector("strong")).toBeNull();
});

test("When Markdown contains links and images then their URLs should remain unchanged", () => {
  const content = [
    "[Subreddit](/r/example)",
    "[External](https://example.com/post)",
    '![Example image](https://images.example.test/example.png "Image title")',
  ].join("\n\n");

  render(<Markdown>{content}</Markdown>);

  expect(screen.getByRole("link", { name: "Subreddit" })).toHaveAttribute("href", "/r/example");
  expect(screen.getByRole("link", { name: "External" })).toHaveAttribute(
    "href",
    "https://example.com/post",
  );
  const image = screen.getByRole("img", { name: "Example image" });
  expect(image).toHaveAttribute("src", "https://images.example.test/example.png");
  expect(image).toHaveAttribute("title", "Image title");
});

test("When Markdown contains raw HTML then it should render as text rather than executable elements", () => {
  const content = '<script>alert("example")</script>\n\n<strong>Not markup</strong>';

  const { container } = render(<Markdown>{content}</Markdown>);

  expect(container.querySelector("script, strong")).toBeNull();
  expect(container).toHaveTextContent('<script>alert("example")</script>');
  expect(container).toHaveTextContent("<strong>Not markup</strong>");
});

test("When a Markdown link uses a javascript URL then the default URL filter should remove it", () => {
  const content = "[Unsafe](javascript:alert%281%29)";

  render(<Markdown>{content}</Markdown>);

  expect(screen.getByText("Unsafe")).not.toHaveAttribute(
    "href",
    expect.stringMatching(/^javascript:/i),
  );
});
