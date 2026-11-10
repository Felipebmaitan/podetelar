// MIT. A deterministic uncompressed source: no codec priming, network or UI clock.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const width = 320,
  height = 180,
  fps = 30,
  seconds = 8,
  sampleRate = 48_000;
const frames = fps * seconds,
  samplesPerFrame = sampleRate / fps;
const frameBytes = width * height * 3,
  blockAlign = 4;
const target = resolve(process.argv[2] ?? ".local/av-sync/flash-click.avi");
const hash = (data) => createHash("sha256").update(data).digest("hex");
function chunk(id, data) {
  const head = Buffer.alloc(8);
  head.write(id, 0, 4, "ascii");
  head.writeUInt32LE(data.length, 4);
  return Buffer.concat([
    head,
    data,
    ...(data.length % 2 ? [Buffer.alloc(1)] : []),
  ]);
}
const list = (type, children) =>
  chunk("LIST", Buffer.concat([Buffer.from(type), ...children]));
function streamHeader(type, scale, rate, length, bufferSize, sampleSize) {
  const header = Buffer.alloc(56);
  header.write(type);
  if (type === "vids") header.write("DIB ", 4);
  header.writeUInt32LE(scale, 20);
  header.writeUInt32LE(rate, 24);
  header.writeUInt32LE(length, 32);
  header.writeUInt32LE(bufferSize, 36);
  header.writeUInt32LE(0xffffffff, 40);
  header.writeUInt32LE(sampleSize, 44);
  if (type === "vids") {
    header.writeUInt16LE(width, 52);
    header.writeUInt16LE(height, 54);
  }
  return header;
}
const main = Buffer.alloc(56);
for (const [offset, value] of [
  [0, Math.round(1e6 / fps)],
  [4, frameBytes * fps + sampleRate * blockAlign],
  [12, 0x110],
  [16, frames],
  [24, 2],
  [28, frameBytes],
  [32, width],
  [36, height],
])
  main.writeUInt32LE(value, offset);
const bitmap = Buffer.alloc(40);
bitmap.writeUInt32LE(40);
bitmap.writeInt32LE(width, 4);
bitmap.writeInt32LE(height, 8);
bitmap.writeUInt16LE(1, 12);
bitmap.writeUInt16LE(24, 14);
bitmap.writeUInt32LE(frameBytes, 20);
const wave = Buffer.alloc(16);
wave.writeUInt16LE(1);
wave.writeUInt16LE(2, 2);
wave.writeUInt32LE(sampleRate, 4);
wave.writeUInt32LE(sampleRate * blockAlign, 8);
wave.writeUInt16LE(blockAlign, 12);
wave.writeUInt16LE(16, 14);
const headers = list("hdrl", [
  chunk("avih", main),
  list("strl", [
    chunk("strh", streamHeader("vids", 1, fps, frames, frameBytes, 0)),
    chunk("strf", bitmap),
  ]),
  list("strl", [
    chunk(
      "strh",
      streamHeader(
        "auds",
        blockAlign,
        sampleRate * blockAlign,
        sampleRate * seconds,
        samplesPerFrame * blockAlign,
        blockAlign,
      ),
    ),
    chunk("strf", wave),
  ]),
]);
const media = [],
  indices = [];
let offset = 4;
function append(id, data) {
  const index = Buffer.alloc(16);
  index.write(id);
  index.writeUInt32LE(0x10, 4);
  index.writeUInt32LE(offset, 8);
  index.writeUInt32LE(data.length, 12);
  const encoded = chunk(id, data);
  media.push(encoded);
  indices.push(index);
  offset += encoded.length;
}
for (let frame = 0; frame < frames; frame++) {
  // White begins at n + 0.5 s for 100 ms. The 20 ms click starts at that same PTS.
  const white = frame % fps >= fps / 2 && frame % fps < fps / 2 + 3;
  append("00db", Buffer.alloc(frameBytes, white ? 255 : 0));
  const audio = Buffer.alloc(samplesPerFrame * blockAlign);
  for (let i = 0; i < samplesPerFrame; i++) {
    const position =
      ((frame * samplesPerFrame + i) % sampleRate) - sampleRate / 2;
    if (position < 0 || position >= 960) continue;
    const envelope = Math.min(1, position / 48, (959 - position) / 48);
    const value = Math.round(
      32767 *
        0.1 *
        envelope *
        Math.sin((2 * Math.PI * 1000 * position) / sampleRate),
    );
    audio.writeInt16LE(value, i * blockAlign);
    audio.writeInt16LE(value, i * blockAlign + 2);
  }
  append("01wb", audio);
}
const file = chunk(
  "RIFF",
  Buffer.concat([
    Buffer.from("AVI "),
    headers,
    list("movi", media),
    chunk("idx1", Buffer.concat(indices)),
  ]),
);
mkdirSync(dirname(target), { recursive: true });
if (existsSync(target)) {
  if (hash(readFileSync(target)) !== hash(file))
    throw new Error(
      "Output exists with different contents; choose another path.",
    );
} else writeFileSync(target, file, { flag: "wx" });
console.log(
  JSON.stringify({
    path: target,
    sha256: hash(file),
    bytes: file.length,
    width,
    height,
    fps,
    seconds,
    sampleRate,
    channels: 2,
    flashAndClickSeconds: Array.from({ length: seconds }, (_, i) => i + 0.5),
    measuredPhysicalOffset: false,
  }),
);
