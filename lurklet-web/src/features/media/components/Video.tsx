import type { VideoSource } from "@/clients/lurklet-api/generated/types.gen";
import { buildMediaUrl } from "../build-media-url";
import { AdaptiveVideo } from "./AdaptiveVideo";
import { Mp4Video } from "./Mp4Video";

type Props = { sources: VideoSource[]; posterUrl: string | null };

export function Video(props: Props) {
  const dash = buildMediaUrl(
    props.sources.find((s) => s.format === "dash" && buildMediaUrl(s.url))?.url,
  );
  const hls = buildMediaUrl(
    props.sources.find((s) => s.format === "hls" && buildMediaUrl(s.url))?.url,
  );
  return dash || hls ? (
    <AdaptiveVideo key={JSON.stringify([dash, hls])} {...props} dash={dash} hls={hls} />
  ) : (
    <Mp4Video {...props} />
  );
}
