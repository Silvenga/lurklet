import type { Image, Root } from "mdast";
import { definitions } from "mdast-util-definitions";
import { visit } from "unist-util-visit";
import type { MarkdownMedia } from "@/clients/lurklet-api/generated/types.gen";

export function remarkMedia(media: MarkdownMedia[]) {
  const references = new Map(media.map((asset) => [asset.id, asset]));

  return (tree: Root) => {
    const definition = definitions(tree);
    visit(tree, (node, index, parent) => {
      let image: Image;
      if (node.type === "image") {
        image = node;
      } else if (node.type === "imageReference" && parent && index !== undefined) {
        const source = definition(node.identifier);
        if (!source) return;
        image = { type: "image", url: source.url, title: source.title, alt: node.alt };
        parent.children[index] = image;
      } else {
        return;
      }

      const asset = references.get(image.url);
      if (!asset) {
        if (!/^(?:[a-z][a-z\d+.-]*:|\/)/i.test(image.url)) image.url = "";
        return;
      }

      image.url = asset.mimeType === null || asset.mimeType.startsWith("image/") ? asset.url : "";
      image.data = {
        ...image.data,
        hProperties: {
          ...image.data?.hProperties,
          ...(asset.width !== null && { width: asset.width }),
          ...(asset.height !== null && { height: asset.height }),
        },
      };
    });
  };
}
