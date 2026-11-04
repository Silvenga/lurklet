import { client } from "@/clients/lurklet-api/generated/client.gen";
import type { GetMediaData } from "@/clients/lurklet-api/generated/types.gen";

export function buildMediaUrl(src: string | null | undefined): string | undefined {
  if (!src) {
    return undefined;
  }
  const url = URL.parse(src);
  if (!url || url.protocol !== "https:" || url.username || url.password || src.includes("#")) {
    return undefined;
  }
  const route: GetMediaData["url"] = "/api/media/{origin}/{path}";

  // Generated path serialization would encode nested slashes and existing percent escapes.
  return client.buildUrl({
    url: route
      .replace("{origin}", () => url.host)
      .replace("{path}", () => url.href.slice(url.origin.length + 1)),
  });
}
