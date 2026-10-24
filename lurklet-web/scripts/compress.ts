import { readdir, readFile, rm, writeFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { brotliCompress, constants, gzip } from "node:zlib";

const COMPRESSIBLE_EXTENSIONS = new Set([".html", ".js", ".css", ".json", ".svg", ".map"]);
const gzipAsync = promisify(gzip);
const brotliCompressAsync = promisify(brotliCompress);

export async function compressAssets(directory: string) {
  const entries = await readdir(directory, { recursive: true, withFileTypes: true });

  for (const entry of entries) {
    if (!entry.isFile() || !COMPRESSIBLE_EXTENSIONS.has(extname(entry.name))) {
      continue;
    }

    const path = join(entry.parentPath, entry.name);
    const source = await readFile(path);
    const [gz, br] = await Promise.all([
      gzipAsync(source, { level: 9 }),
      brotliCompressAsync(source, {
        params: {
          [constants.BROTLI_PARAM_QUALITY]: 11,
          [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT,
        },
      }),
    ]);

    for (const [extension, compressed] of [
      ["gz", gz],
      ["br", br],
    ] as const) {
      const sidecar = `${path}.${extension}`;

      if (compressed.byteLength < source.byteLength) {
        await writeFile(sidecar, compressed);
      } else {
        await rm(sidecar, { force: true });
      }
    }
  }
}

if (import.meta.main) {
  await compressAssets(fileURLToPath(new URL("../dist", import.meta.url)));
}
