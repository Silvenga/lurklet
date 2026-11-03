import remarkParse from "remark-parse";
import { unified } from "unified";
import { expect, test } from "vitest";
import { remarkGfmParse } from "./remark-gfm-parse";
import { remarkRedditStrikethrough } from "./remark-reddit-strikethrough";

// https://www.reddit.com/r/tasker/comments/hjtoer/project_markdown_anywhere/fwqsk5v
test.each([false, true])(
  "When Reddit strikethrough runs with GFM %s then only double tildes should strike text",
  (gfm) => {
    const parser = unified().use(remarkParse);
    if (gfm) parser.use(remarkGfmParse);
    parser.use(remarkRedditStrikethrough);
    const content = "~single tilde~\n\n~~double tilde~~\n\n\\~~escaped~~\n\n`~~code~~`";

    const tree = parser.parse(content);

    expect(tree.children).toMatchObject([
      { children: [{ type: "text", value: "~single tilde~" }] },
      { children: [{ type: "delete", children: [{ value: "double tilde" }] }] },
      { children: [{ type: "text", value: "~~escaped~~" }] },
      { children: [{ type: "inlineCode", value: "~~code~~" }] },
    ]);
  },
);
