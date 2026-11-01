import remarkParse from "remark-parse";
import { unified } from "unified";
import { expect, test } from "vitest";
import { remarkRedditSuperscript } from "./remark-reddit-superscript";

const parser = unified().use(remarkParse).use(remarkRedditSuperscript);

// https://www.reddit.com/r/LearnToReddit/comments/v6d66y/its_a_learntoreddit_challenge/icv3l6g
// https://www.reddit.com/r/LearnToReddit/comments/v6d66y/its_a_learntoreddit_challenge/ibexke6
test("When superscript uses a word or a parenthesized phrase then it should stop at the correct boundary", () => {
  const content = "^Sunday NIGHT\n\n^(Yeah, Monday!)";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      children: [
        { type: "redditSuperscript", children: [{ value: "Sunday" }] },
        { type: "text", value: " NIGHT" },
      ],
    },
    {
      children: [
        {
          type: "redditSuperscript",
          data: { hName: "sup" },
          children: [{ value: "Yeah, Monday!" }],
        },
      ],
    },
  ]);
});

test("When a superscript phrase contains formatting parentheses escapes and code then it should parse them without ending early", () => {
  const content = "^(**bold** [link](https://example.com/a_(b)) (aside) \\) `)` end)";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      children: [
        {
          type: "redditSuperscript",
          children: [
            { type: "strong", children: [{ value: "bold" }] },
            { type: "text", value: " " },
            { type: "link", url: "https://example.com/a_(b)" },
            { type: "text", value: " (aside) ) " },
            { type: "inlineCode", value: ")" },
            { type: "text", value: " end" },
          ],
        },
      ],
    },
  ]);
});

test("When superscript is escaped malformed or in code then it should remain literal", () => {
  const content =
    "\\^(escaped)\n\n^() ^(unclosed\n\n`^inline`\n\n    ^indented\n\n```\n^(fenced)\n```";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    { children: [{ type: "text", value: "^(escaped)" }] },
    { children: [{ type: "text", value: "^() ^(unclosed" }] },
    { children: [{ type: "inlineCode", value: "^inline" }] },
    { type: "code", value: "^indented" },
    { type: "code", value: "^(fenced)" },
  ]);
});

test("When a bare superscript meets emphasis or a multiline code span then it should not consume the enclosing syntax", () => {
  const content = "*^word*\n\n^`text\nmore`";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      children: [
        {
          type: "emphasis",
          children: [{ type: "redditSuperscript", children: [{ value: "word" }] }],
        },
      ],
    },
    {
      children: [
        { type: "text", value: "^" },
        { type: "inlineCode", value: "text\nmore" },
      ],
    },
  ]);
});
