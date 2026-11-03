import type { PostFlair as Flair } from "@/clients/lurklet-api/generated/types.gen";
import { Badge } from "@/components/ui/badge";
import { Media } from "@/features/media/components/Media";

export function PostFlair({ flair }: { flair: Flair | null }) {
  if (!flair) {
    return null;
  }

  return (
    <Badge
      variant="secondary"
      className="h-auto min-h-5 max-w-full min-w-0 shrink py-0 font-normal whitespace-normal select-text"
      style={{
        backgroundColor:
          flair.backgroundColor && /^#[0-9a-f]{6}$/i.test(flair.backgroundColor)
            ? flair.backgroundColor
            : undefined,
        color:
          flair.textColor === "light"
            ? "#ffffff"
            : flair.textColor === "dark"
              ? "#000000"
              : undefined,
      }}
    >
      <span className="sr-only">Flair: </span>
      <span className="min-w-0 wrap-anywhere whitespace-pre-wrap">
        {flair.parts.length > 0
          ? flair.parts.map((part, index) =>
              part.kind === "text" ? (
                part.text
              ) : (
                <Media key={index} variant="emoji" kind="image" src={part.url} alt={part.alt} />
              ),
            )
          : flair.text}
      </span>
    </Badge>
  );
}
