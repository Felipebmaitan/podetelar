// Fixed raw RGB sources are shared across codecs/presets; decoding is verified.
import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  createReadStream,
  mkdirSync,
  readFileSync,
  writeFileSync,
  statSync,
  openSync,
  readSync,
  closeSync,
  unlinkSync,
} from "node:fs";
import { resolve, join, sep } from "node:path";
const root = resolve(import.meta.dirname, "..");
const run = join(root, ".local/experiments", `nvidia-quality-${randomUUID()}`);
mkdirSync(run, { recursive: true });
const exe = join(root, "target/release/examples/pipeline-resource.exe");
const pathForGst = (path) => path.replaceAll("\\", "/");
async function sha(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}
async function pipeline(label, description) {
  const request = join(run, `${label}-request.json`),
    report = join(run, `${label}.json`);
  writeFileSync(request, JSON.stringify({ description }));
  const p = spawn(exe, [request, report], {
    windowsHide: true,
    stdio: ["ignore", "ignore", "ignore"],
    env: {
      ...process.env,
      PATH: `${join(root, ".local/gstreamer/bin")};${process.env.PATH}`,
    },
  });
  const timer = setTimeout(() => p.kill(), 60000);
  const code = await new Promise((resolve) => {
    p.once("error", () => resolve(-1));
    p.once("exit", resolve);
  });
  clearTimeout(timer);
  if (code !== 0) throw new Error(`Local quality pipeline failed: ${label}`);
  const result = JSON.parse(readFileSync(report));
  if (!result.completed || !result.valid)
    throw new Error("Invalid local quality pipeline.");
  return result;
}
function compare(reference, decoded) {
  const size = statSync(reference).size;
  if (statSync(decoded).size !== size)
    throw new Error("Decoded RGB dimensions/frame count changed.");
  const a = openSync(reference, "r"),
    b = openSync(decoded, "r"),
    left = Buffer.allocUnsafe(1024 * 1024),
    right = Buffer.allocUnsafe(left.length);
  let squared = 0,
    position = 0;
  try {
    while (position < size) {
      const wanted = Math.min(size - position, left.length);
      const n = readSync(a, left, 0, wanted, position),
        m = readSync(b, right, 0, wanted, position);
      if (n !== wanted || m !== wanted)
        throw new Error("Truncated quality fixture.");
      for (let i = 0; i < n; i++) {
        const d = left[i] - right[i];
        squared += d * d;
      }
      position += n;
    }
  } finally {
    closeSync(a);
    closeSync(b);
  }
  const mse = squared / size;
  return {
    rgbBytes: size,
    mse,
    psnrDb: mse ? 10 * Math.log10((255 * 255) / mse) : null,
  };
}
const rows = [];
const frameBytes = 1920 * 1080 * 3,
  frames = 30;
for (const pattern of ["ball", "snow"]) {
  const reference = join(run, `${pattern}-source.rgb`);
  await pipeline(
    `${pattern}-source`,
    `videotestsrc num-buffers=${frames} pattern=${pattern} ! video/x-raw,format=RGB,width=1920,height=1080,framerate=60/1 ! filesink location="${pathForGst(reference)}" sync=false`,
  );
  if (statSync(reference).size !== frameBytes * frames)
    throw new Error("Raw reference geometry is wrong.");
  const referenceSha256 = await sha(reference);
  for (const codec of ["h264", "h265", "av1"])
    for (const preset of ["p1", "p4", "p7"]) {
      const label = `${pattern}-${codec}-${preset}`,
        decoded = join(run, `${label}.rgb`),
        encoded = join(run, `${label}.${codec}`);
      const profile = codec === "h264" ? "constrained-baseline" : "main";
      const parser = `${codec}parse${codec === "av1" ? "" : " config-interval=-1"}`;
      const description = `filesrc location="${pathForGst(reference)}" ! rawvideoparse format=rgb width=1920 height=1080 framerate=60/1 ! videoconvert ! video/x-raw,format=NV12 ! d3d11upload ! video/x-raw(memory:D3D11Memory),format=NV12 ! nvd3d11${codec}enc preset=${preset} tune=ultra-low-latency rc-mode=cbr bitrate=6000 gop-size=60 bframes=0 zerolatency=true ! video/x-${codec},profile=${profile} ! ${parser} ! tee name=coded coded. ! queue ! filesink location="${pathForGst(encoded)}" sync=false coded. ! queue ! d3d11${codec}dec ! d3d11download ! videoconvert ! video/x-raw,format=RGB ! identity name=count ! filesink location="${pathForGst(decoded)}" sync=false`;
      const result = await pipeline(label, description);
      if (result.deliveredBuffers !== frames)
        throw new Error("Quality comparison lost decoded frames.");
      rows.push({
        pattern,
        codec,
        preset,
        width: 1920,
        height: 1080,
        fps: 60,
        frames,
        targetKbps: 6000,
        referenceSha256,
        decodedSha256: await sha(decoded),
        encodedSha256: await sha(encoded),
        encodedBytes: statSync(encoded).size,
        ...compare(reference, decoded),
      });
      const resolved = resolve(decoded);
      if (!resolved.startsWith(resolve(run) + sep))
        throw new Error("Unexpected scratch media path.");
      unlinkSync(resolved);
      console.log(
        JSON.stringify({ pattern, codec, preset, verifiedFrames: frames }),
      );
    }
}
const output = join(run, "summary.json");
writeFileSync(
  output,
  JSON.stringify(
    {
      schema: 1,
      runs: rows,
      limits:
        "Actual local NVIDIA encode/decode, fixed identical raw source per pattern. Full RGB byte PSNR includes common RGB/YUV conversion losses. Thirty ball/snow frames are a synthetic quality probe, not perceptual game quality, steady-state rate control, physical 4K120 or AMD qualification. No personal capture/media.",
    },
    null,
    2,
  ),
);
console.log(JSON.stringify({ output }));
