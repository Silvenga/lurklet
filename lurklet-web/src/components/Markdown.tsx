import { cn } from "cn";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Markdown as MarkdownValue } from "@/clients/lurklet-api/generated/types.gen";
import { MarkdownLink } from "@/components/markdown/MarkdownLink";
import { remarkMedia } from "@/components/markdown/remark-media";
import { Media } from "@/features/media/components/Media";

const COMPONENTS: Components = { a: MarkdownLink, img: Media };

export function Markdown({ value, className }: { value: MarkdownValue; className?: string }) {
  return (
    <div className={cn("prose dark:prose-invert", className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm, [remarkMedia, value.media]]}
        components={COMPONENTS}
      >
        {value.content}
      </ReactMarkdown>
    </div>
  );
}
