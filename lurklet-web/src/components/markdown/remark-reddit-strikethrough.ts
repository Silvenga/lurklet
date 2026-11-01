import { gfmStrikethroughFromMarkdown } from "mdast-util-gfm-strikethrough";
import { gfmStrikethrough } from "micromark-extension-gfm-strikethrough";
import type {} from "remark-parse";
import type { Processor } from "unified";

export function remarkRedditStrikethrough(this: Processor) {
  const data = this.data();
  const syntax = gfmStrikethrough({ singleTilde: false });
  const constructs = syntax.text?.[126];
  const construct = Array.isArray(constructs) ? constructs[0] : constructs;
  (data.micromarkExtensions ??= []).push({
    ...syntax,
    disable: { null: ["strikethrough"] },
    text: { 126: { ...construct!, name: "redditStrikethrough" } },
  });
  (data.fromMarkdownExtensions ??= []).push(gfmStrikethroughFromMarkdown());
}
