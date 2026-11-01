import remarkParse from "remark-parse";
import { unified } from "unified";
import { expect, test } from "vitest";
import { remarkRedditSpoilers } from "./remark-reddit-spoilers";

const parser = unified().use(remarkParse).use(remarkRedditSpoilers);

// https://www.reddit.com/r/interesting/comments/1sfkkql/i_wrote_down_all_reddit_text_tricks_so_everyone/oeyln8s
test("When a line starts with a spoiler then it should not become a blockquote and its escaped counterpart should stay literal", () => {
  const content = ">!Spoiler!< \\>!Spoiler!<";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      type: "paragraph",
      children: [
        {
          type: "redditSpoiler",
          children: [{ value: "Spoiler" }],
          data: { hName: "span", hProperties: { "data-reddit-spoiler": true } },
        },
        { type: "text", value: " >!Spoiler!<" },
      ],
    },
  ]);
});

test("When spoilers contain inline formatting and literal delimiters then they should preserve their contents", () => {
  const content = "Before >!**bold** [link](https://example.com) `!<` \\!< after!< end";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      children: [
        { type: "text", value: "Before " },
        {
          type: "redditSpoiler",
          children: [
            { type: "strong", children: [{ value: "bold" }] },
            { type: "text", value: " " },
            { type: "link", url: "https://example.com" },
            { type: "text", value: " " },
            { type: "inlineCode", value: "!<" },
            { type: "text", value: " !< after" },
          ],
        },
        { type: "text", value: " end" },
      ],
    },
  ]);
});

test("When spoilers occur in lists and quotes then ordinary containers should remain intact", () => {
  const content = "1. Joke >!Punchline!<\n\n> Quote\n> >!Secret!<\n> Continued\n\n>!one\nline!<";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      type: "list",
      children: [{ children: [{ children: [{ value: "Joke " }, { type: "redditSpoiler" }] }] }],
    },
    {
      type: "blockquote",
      children: [
        {
          children: [
            { value: "Quote\n" },
            { type: "redditSpoiler", children: [{ value: "Secret" }] },
            { value: "\nContinued" },
          ],
        },
      ],
    },
    {
      type: "paragraph",
      children: [{ type: "redditSpoiler", children: [{ value: "one\nline" }] }],
    },
  ]);
});

test("When spoilers are unmatched empty or in code then they should remain literal", () => {
  const content = ">!unclosed\n\n>!!<\n\n`>!inline!<`\n\n    >!indented!<\n\n```\n>!fenced!<\n```";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    { type: "paragraph", children: [{ value: ">!unclosed" }] },
    { type: "paragraph", children: [{ value: ">!!<" }] },
    { children: [{ type: "inlineCode", value: ">!inline!<" }] },
    { type: "code", value: ">!indented!<" },
    { type: "code", value: ">!fenced!<" },
  ]);
});

test("When a spoiler closes on its own line after inline code then parsing should preserve the multiline chunk boundaries", () => {
  const content = "**>!##\\ `~~a>!r/rust\\r/rust`\n!<";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      children: [
        { type: "text", value: "**" },
        {
          type: "redditSpoiler",
          children: [
            { type: "text", value: "##\\ " },
            { type: "inlineCode", value: "~~a>!r/rust\\r/rust" },
            { type: "text", value: "\n" },
          ],
        },
      ],
    },
  ]);
});
