import remarkParse from "remark-parse";
import { unified } from "unified";
import { visit } from "unist-util-visit";
import { expect, test } from "vitest";
import { remarkRedditAutolinks } from "./remark-reddit-autolinks";

const parser = unified().use(remarkParse).use(remarkRedditAutolinks);

// https://www.reddit.com/r/LearnToReddit/comments/qgk0nt
test("When prose contains Reddit mentions then all four forms should become local links with punctuation preserved", () => {
  const content = "For example r/NewToReddit or u/llamageddon01. (/r/rust), /u/Rich_D_sr!";

  const tree = parser.runSync(parser.parse(content));

  expect(tree.children).toMatchObject([
    {
      children: [
        { value: "For example " },
        { type: "link", url: "/r/NewToReddit", children: [{ value: "r/NewToReddit" }] },
        { value: " or " },
        { type: "link", url: "/u/llamageddon01" },
        { value: ". (" },
        { type: "link", url: "/r/rust" },
        { value: "), " },
        { type: "link", url: "/u/Rich_D_sr" },
        { value: "!" },
      ],
    },
  ]);
});

test("When mentions occur inside links code escaped text or URLs then they should not create additional links", () => {
  const content =
    "[**r/rust**](https://example.com) [u/example][ref]\n\n[ref]: /original\n\n`r/rust`\n\n    /u/example\n\n```\nr/rust\n```\n\nr\\/rust \\/u/example https://example.com/r/rust another/r/rust /r/rust/wiki r/ u/";

  const tree = parser.runSync(parser.parse(content));
  const links: string[] = [];
  visit(tree, "link", (node) => {
    links.push(node.url);
  });

  expect(links).toEqual(["https://example.com"]);
  expect(tree.children[0]).toMatchObject({
    children: [
      {
        type: "link",
        children: [{ type: "strong", children: [{ type: "text", value: "r/rust" }] }],
      },
      { value: " " },
      { type: "linkReference", children: [{ type: "text", value: "u/example" }] },
    ],
  });
});
