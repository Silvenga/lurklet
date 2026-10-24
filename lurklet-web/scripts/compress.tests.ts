// @vitest-environment node

import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { brotliDecompressSync, gunzipSync } from "node:zlib";
import { afterEach, beforeEach, expect, test } from "vitest";
import { compressAssets } from "./compress.ts";

let directory: string;

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), "lurklet-web-compress-"));
});

afterEach(async () => {
  await rm(directory, { recursive: true, force: true });
});

test.each(["html", "js", "css", "json", "svg", "map"])(
  "When a nested %s asset is compressible then both sidecars should round-trip to the original",
  async (extension) => {
    const assets = join(directory, "assets");
    await mkdir(assets);
    const path = join(assets, `asset.${extension}`);
    const source = Buffer.from("Compressible source content.\n".repeat(100));
    await writeFile(path, source);

    await compressAssets(directory);

    const gz = await readFile(`${path}.gz`);
    const br = await readFile(`${path}.br`);
    expect(gunzipSync(gz)).toEqual(source);
    expect(brotliDecompressSync(br)).toEqual(source);
    expect(gz.byteLength).toBeLessThan(source.byteLength);
    expect(br.byteLength).toBeLessThan(source.byteLength);
    expect(await readFile(path)).toEqual(source);
  },
);

test("When assets are already compressed or unsupported then they should remain unchanged", async () => {
  const names = ["font.woff2", "image.png", "asset.js.gz", "asset.js.br"];
  const source = Buffer.from("Compressible but excluded content.\n".repeat(100));
  for (const name of names) {
    await writeFile(join(directory, name), source);
  }

  await compressAssets(directory);

  expect((await readdir(directory)).sort()).toEqual(names.toSorted());
  for (const name of names) {
    expect(await readFile(join(directory, name))).toEqual(source);
  }
});

test("When compression is not smaller then sidecars should not be retained", async () => {
  const path = join(directory, "small.js");
  await writeFile(path, "x");
  await writeFile(`${path}.gz`, "stale gzip");
  await writeFile(`${path}.br`, "stale brotli");

  await compressAssets(directory);

  expect(await readdir(directory)).toEqual(["small.js"]);
  expect(await readFile(path, "utf8")).toBe("x");
});

test("When compression runs again then sidecars should reflect the current source", async () => {
  const path = join(directory, "asset.js");
  await writeFile(path, "Original content.\n".repeat(100));
  await compressAssets(directory);
  const source = Buffer.from("Updated content.\n".repeat(100));
  await writeFile(path, source);

  await compressAssets(directory);

  expect(gunzipSync(await readFile(`${path}.gz`))).toEqual(source);
  expect(brotliDecompressSync(await readFile(`${path}.br`))).toEqual(source);
  expect((await readdir(directory)).sort()).toEqual(["asset.js", "asset.js.br", "asset.js.gz"]);
});

test("When the build output is missing then compression should fail", async () => {
  const missing = join(directory, "missing");

  const result = compressAssets(missing);

  await expect(result).rejects.toMatchObject({ code: "ENOENT" });
});
