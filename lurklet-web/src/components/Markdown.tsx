import { cn } from "cn";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Markdown as MarkdownValue } from "@/clients/lurklet-api/generated/types.gen";
import { MarkdownLink } from "@/components/markdown/MarkdownLink";
import { MarkdownSpoiler } from "@/components/markdown/MarkdownSpoiler";
import { remarkMedia } from "@/components/markdown/remark-media";
import { remarkRedditAutolinks } from "@/components/markdown/remark-reddit-autolinks";
import { remarkRedditHeadings } from "@/components/markdown/remark-reddit-headings";
import { remarkRedditSpoilers } from "@/components/markdown/remark-reddit-spoilers";
import { remarkRedditStrikethrough } from "@/components/markdown/remark-reddit-strikethrough";
import { remarkRedditSuperscript } from "@/components/markdown/remark-reddit-superscript";
import { Media } from "@/features/media/components/Media";

const COMPONENTS: Components = {
  a: MarkdownLink,
  img: Media,
  span: ({ node, children, ...props }) =>
    node?.properties["data-reddit-spoiler"] ? (
      <MarkdownSpoiler>{children}</MarkdownSpoiler>
    ) : (
      <span {...props}>{children}</span>
    ),
};

export function Markdown({ value, className }: { value: MarkdownValue; className?: string }) {
  return (
    <div className={cn("prose dark:prose-invert wrap-anywhere", className)}>
      <ReactMarkdown
        remarkPlugins={[
          remarkGfm,
          remarkRedditHeadings,
          remarkRedditSpoilers,
          remarkRedditSuperscript,
          remarkRedditAutolinks,
          remarkRedditStrikethrough,
          [remarkMedia, value.media],
        ]}
        components={COMPONENTS}
      >
        {value.content}
      </ReactMarkdown>
    </div>
  );
}
