import { expect, test } from "vitest";
import { VIDEO_SOURCE_URL } from "./fixtures";

test("When the offline MP4 is requested then full partial conditional and HEAD responses should describe the same bytes", async () => {
  const url = new URL(`/api/media/${VIDEO_SOURCE_URL.slice("https://".length)}`, location.origin);
  const response = await fetch(url);
  const bytes = new Uint8Array(await response.arrayBuffer());

  expect(response.status).toBe(200);
  expect(response.headers.get("Content-Type")).toBe("video/mp4");
  expect(new TextDecoder().decode(bytes.slice(4, 8))).toBe("ftyp");
  for (const [range, start, end] of [
    ["bytes=0-3", 0, 3],
    [`bytes=${bytes.length - 4}-`, bytes.length - 4, bytes.length - 1],
    ["bytes=-4", bytes.length - 4, bytes.length - 1],
  ] as const) {
    const partial = await fetch(url, { headers: { Range: range } });

    expect(partial.status).toBe(206);
    expect(partial.headers.get("Content-Range")).toBe(`bytes ${start}-${end}/${bytes.length}`);
    expect(partial.headers.get("Content-Length")).toBe("4");
    expect(new Uint8Array(await partial.arrayBuffer())).toEqual(bytes.slice(start, end + 1));
  }

  const head = await fetch(url, { method: "HEAD", headers: { Range: "bytes=0-3" } });
  const conditional = await fetch(url, { headers: { Range: "bytes=0-3", "If-Range": '"old"' } });
  const unsatisfiable = await fetch(url, { headers: { Range: `bytes=${bytes.length}-` } });

  expect(head.status).toBe(200);
  expect(head.headers.get("Content-Length")).toBe(String(bytes.length));
  expect((await head.arrayBuffer()).byteLength).toBe(0);
  expect(conditional.status).toBe(200);
  expect(new Uint8Array(await conditional.arrayBuffer())).toEqual(bytes);
  expect(unsatisfiable.status).toBe(416);
  expect(unsatisfiable.headers.get("Content-Range")).toBe(`bytes */${bytes.length}`);
});
