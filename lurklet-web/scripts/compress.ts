import { readdir, readFile, rm, writeFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { brotliCompress, constants, gzip, zstdCompress } from "node:zlib";

const COMPRESSIBLE_EXTENSIONS = new Set([".html", ".js", ".css", ".json", ".svg", ".map"]);
const gzipAsync = promisify(gzip);
const brotliCompressAsync = promisify(brotliCompress);
const zstdCompressAsync = promisify(zstdCompress);

export async function compressAssets(directory: string) {
  const entries = await readdir(directory, { recursive: true, withFileTypes: true });

  for (const entry of entries) {
    if (!entry.isFile() || !COMPRESSIBLE_EXTENSIONS.has(extname(entry.name))) {
      continue;
    }

    const path = join(entry.parentPath, entry.name);
    const source = await readFile(path);
    const [gz, br, zst] = await Promise.all([
      gzipAsync(source, { level: 9 }),
      brotliCompressAsync(source, {
        params: {
          [constants.BROTLI_PARAM_QUALITY]: 11,
          [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT,
        },
      }),
      zstdCompressAsync(source, {
        params: {
          [constants.ZSTD_c_compressionLevel]: 19,
          // RFC 9659 limits HTTP zstd windows to 8 MiB.
          [constants.ZSTD_c_windowLog]: 23,
        },
      }),
    ]);

    for (const [extension, compressed] of [
      ["gz", gz],
      ["br", br],
      ["zst", zst],
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
