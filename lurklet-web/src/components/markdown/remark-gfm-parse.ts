import { gfmFromMarkdown } from "mdast-util-gfm";
import { gfm } from "micromark-extension-gfm";
import type {} from "remark-parse";
import type { Processor } from "unified";

// remark-gfm registers serialization handlers that ReactMarkdown never uses.
// Registering only parsing extensions lets the bundler remove that serialization code.
export function remarkGfmParse(this: Processor) {
  const data = this.data();
  (data.micromarkExtensions ??= []).push(gfm());
  (data.fromMarkdownExtensions ??= []).push(gfmFromMarkdown());
}
