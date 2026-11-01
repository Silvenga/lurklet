import remarkParse from "remark-parse";
import { unified } from "unified";
import { expect, test } from "vitest";
import { remarkRedditHeadings } from "./remark-reddit-headings";

const parser = unified().use(remarkParse).use(remarkRedditHeadings);

// https://www.reddit.com/r/DataHoarder/comments/1u5ljvs
test("When a Reddit heading omits whitespace then it should retain its level and inline formatting", () => {
  const content = "##Regarding HDD price posts:  \n\n###**AI content**";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    {
      type: "heading",
      depth: 2,
      children: [{ type: "text", value: "Regarding HDD price posts:" }],
    },
    {
      type: "heading",
      depth: 3,
      children: [{ type: "strong", children: [{ value: "AI content" }] }],
    },
  ]);
});

test("When headings interrupt paragraphs or occur in containers then they should preserve block structure", () => {
  const content = "Paragraph\n#Heading\n\n> ##Quote ##\n\n- ###List\n\n######Six\n\n#######Seven";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    { type: "paragraph" },
    { type: "heading", depth: 1, children: [{ value: "Heading" }] },
    {
      type: "blockquote",
      children: [{ type: "heading", depth: 2, children: [{ value: "Quote" }] }],
    },
    { type: "list", children: [{ children: [{ type: "heading", depth: 3 }] }] },
    { type: "heading", depth: 6 },
    { type: "paragraph", children: [{ value: "#######Seven" }] },
  ]);
});

test("When heading markers are escaped or in code then they should remain literal", () => {
  const content = "\\#Heading\n\n`#Inline`\n\n    ##Indented\n\n```\n###Fenced\n```";

  const tree = parser.parse(content);

  expect(tree.children).toMatchObject([
    { type: "paragraph", children: [{ type: "text", value: "#Heading" }] },
    { type: "paragraph", children: [{ type: "inlineCode", value: "#Inline" }] },
    { type: "code", value: "##Indented" },
    { type: "code", value: "###Fenced" },
  ]);
});
