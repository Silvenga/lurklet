import { cn } from "cn";
import { EyeIcon } from "lucide-react";
import { useState } from "react";
import type { Thumbnail } from "@/clients/lurklet-api/generated/types.gen";
import { Button } from "@/components/ui/button";
import { Media } from "@/features/media/components/Media";

export function PostThumbnail({
  thumbnail,
  title,
  isSensitive,
}: {
  thumbnail: Thumbnail;
  title: string;
  isSensitive: boolean;
}) {
  const [isRevealed, setIsRevealed] = useState(false);
  const isHidden = isSensitive && !isRevealed;

  return (
    <div className="relative size-16 shrink-0 select-none">
      <div
        aria-hidden={isHidden}
        className={cn("overflow-hidden rounded-lg", isHidden && "[&_img]:blur-md")}
      >
        <Media
          kind="image"
          variant="thumbnail"
          src={isHidden ? thumbnail.obfuscatedUrl || thumbnail.url : thumbnail.url}
          alt={`Thumbnail for ${title}`}
        />
      </div>
      {isHidden && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="absolute inset-0 size-full"
          aria-label={`Reveal thumbnail for ${title}`}
          onPress={() => setIsRevealed(true)}
        >
          <EyeIcon aria-hidden="true" data-icon="inline-start" />
        </Button>
      )}
    </div>
  );
}
