import { cn } from "cn";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { MarkdownLink } from "@/components/markdown/MarkdownLink";
import { Media } from "@/features/media/components/Media";

const COMPONENTS: Components = { a: MarkdownLink, img: Media };

export function Markdown({ children, className }: { children: string; className?: string }) {
  return (
    <div className={cn("prose dark:prose-invert", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
        {children}
      </ReactMarkdown>
    </div>
  );
}
