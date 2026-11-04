import type { PostAttachment } from "@/clients/lurklet-api/generated/types.gen";
import { ContentLink } from "@/components/ContentLink";
import { Media } from "@/features/media/components/Media";
import { Video } from "@/features/media/components/Video";

export function PostAttachments({ attachments }: { attachments: PostAttachment[] }) {
  return (
    <>
      {attachments.map((attachment, index) => {
        switch (attachment.kind) {
          case "link":
            return (
              <ContentLink
                key={index}
                className="wrap-anywhere underline underline-offset-4"
                href={attachment.url}
              >
                {attachment.url}
              </ContentLink>
            );
          case "image":
            return (
              <figure key={index} className="flex flex-col gap-2">
                <Media kind="image" src={attachment.url} alt={attachment.altText ?? undefined} />
                {attachment.caption && (
                  <figcaption className="text-muted-foreground text-sm wrap-anywhere">
                    {attachment.caption}
                  </figcaption>
                )}
              </figure>
            );
          case "video":
            return (
              <Video key={index} sources={attachment.sources} posterUrl={attachment.posterUrl} />
            );
        }
      })}
    </>
  );
}
