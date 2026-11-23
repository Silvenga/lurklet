import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import pngToIco from "png-to-ico";
import sharp from "sharp";

const directory = new URL("../static/public/", import.meta.url);
const logo = await readFile(new URL("logo.svg", directory));
const { width } = await sharp(logo).metadata();

function render(size: number) {
  return sharp(logo, { density: Math.ceil((72 * size) / width) }).resize(size, size);
}

const frames = await Promise.all([16, 32, 48].map((size) => render(size).png().toBuffer()));
await writeFile(new URL("../favicon.ico", directory), await pngToIco(frames));

for (const [size, name] of [
  [180, "apple-touch-icon.png"],
  [192, "icon-192.png"],
  [512, "icon-512.png"],
] as const) {
  const padding = Math.round(size / 8);
  await render(size - 2 * padding)
    .flatten({ background: "white" })
    .extend({ top: padding, bottom: padding, left: padding, right: padding, background: "white" })
    .png()
    .toFile(fileURLToPath(new URL(name, directory)));
}
