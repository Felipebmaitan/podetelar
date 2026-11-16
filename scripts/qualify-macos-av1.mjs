// Real pre-encoded AV1 -> native SFU -> macOS hardware decoder. No AV1 encoder ships.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { root, macosEnvironment } from "./macos-runtime.mjs";
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("Use Apple Silicon macOS.");
const latest = JSON.parse(
  readFileSync(join(root, ".local/macos/latest-release.json")),
);
const app = process.env.MACOS_TEST_APP || latest.app;
const receiver =
  process.env.MACOS_TEST_RECEIVER ||
  join(app, "Contents/Helpers/PodeTelar.app/Contents/MacOS/FreedomStream");
const sender = join(root, ".local/macos/fixtures/freedomstream-publisher");
const fixturePath = join(root, ".local/macos/av1-fixture.ivf");
const packets = JSON.parse(
  execFileSync(
    "ffprobe",
    [
      "-v",
      "error",
      "-select_streams",
      "v:0",
      "-show_entries",
      "packet=flags",
      "-of",
      "json",
      fixturePath,
    ],
    { encoding: "utf8", maxBuffer: 1024 * 1024 },
  ),
).packets;
const keyFrames = packets.flatMap((packet, index) =>
  packet.flags.includes("K") ? [index] : [],
);
const receiverEnv = process.env.MACOS_TEST_RECEIVER
  ? macosEnvironment()
  : {
      ...process.env,
      DYLD_LIBRARY_PATH: "",
      DYLD_FALLBACK_LIBRARY_PATH: "",
      GST_PLUGIN_SYSTEM_PATH_1_0: join(app, "Contents/PlugIns"),
      GST_PLUGIN_PATH_1_0: "",
      GST_PLUGIN_SCANNER_1_0: join(app, "Contents/MacOS/gst-plugin-scanner"),
      GST_REGISTRY_1_0: join(root, ".local/macos/av1-registry.bin"),
    };
function native(executable, option, request, env) {
  const child = spawn(executable, [option], {
    env: { ...env, GST_DEBUG: "vtdec:5" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let output = "",
    error = "";
  child.stdout.on("data", (chunk) => {
    output += chunk;
    if (output.length > 4 * 1024 * 1024) child.kill();
  });
  child.stderr.on("data", (chunk) => {
    if (error.length < 1024 * 1024) error += chunk;
  });
  child.stdin.end(JSON.stringify(request));
  const done = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("AV1 qualification timed out."));
    }, 45000);
    child.once("error", () => {
      clearTimeout(timer);
      reject(new Error("Could not start AV1 fixture."));
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      if (option === "--watch-fixture")
        writeFileSync(
          join(root, ".local/macos/av1-decoder.log"),
          error
            .split(/\r?\n/)
            .filter((line) => line.includes("vtdec"))
            .join("\n"),
        );
      if (code !== 0) {
        reject(
          new Error(
            `AV1 fixture failed: ${error.split(/\r?\n/).find((line) => line.startsWith("PodeTelar:")) ?? code}`,
          ),
        );
        return;
      }
      try {
        resolve(
          output
            .split(/\r?\n/)
            .filter((line) => line.startsWith("{"))
            .map((line) => JSON.parse(line)),
        );
      } catch {
        reject(new Error("Invalid AV1 fixture output."));
      }
    });
  });
  return { child, done };
}
const origin = process.env.MACOS_TEST_ORIGIN || "http://localhost:3010";
const response = await fetch(`${origin}/api/rooms`, {
  method: "POST",
  headers: {
    "Content-Type": "application/json",
    Origin: origin,
    Authorization: `Bearer ${process.env.ADMIN_TOKEN}`,
  },
  body: JSON.stringify({
    name: "macOS AV1 qualification",
    displayName: "Test owner",
  }),
  signal: AbortSignal.timeout(5000),
});
assert.equal(response.status, 201, "Could not create isolated AV1 test room.");
const { invitationUrl } = await response.json();
const watching = native(
  receiver,
  "--watch-fixture",
  { invitationUrl, seconds: 18, consumePictures: true, diagnostics: true },
  receiverEnv,
);
const publishing = native(
  sender,
  "--publish-av1-fixture",
  {
    invitationUrl,
    seconds: 18,
    fixturePath,
    keyFrames,
  },
  macosEnvironment(),
);
try {
  const [viewed, published] = await Promise.all([
    watching.done,
    publishing.done,
  ]);
  const final = viewed.find((value) => value.closed);
  writeFileSync(
    join(root, ".local/macos/av1-watch.json"),
    JSON.stringify(final, null, 2),
  );
  assert.ok(published[0]?.accepted > 400, "AV1 sender did not deliver frames.");
  assert.ok(
    final?.samples.some((sample) =>
      sample.decoders.some(
        (decoder) =>
          decoder.hardware &&
          decoder.implementation === "PodeTelar VideoToolbox",
      ),
    ),
    "No hardware AV1 decoder reported.",
  );
  assert.ok(
    final.samples.some((sample) =>
      sample.streams.some(
        (stream) =>
          stream.codec.toLowerCase().includes("av1") && stream.frames > 30,
      ),
    ),
    "No AV1 pictures presented.",
  );
  const stopped = final.samples.filter(
    (sample) => sample.phaseSecond >= 6 && sample.phaseSecond < 8,
  );
  assert.ok(stopped.length >= 2);
  const stoppedBytes = stopped.at(-1).receivedBytes - stopped[0].receivedBytes;
  assert.equal(stoppedBytes, 0, "Stop watching retained AV1 traffic.");
  assert.ok(
    final.samples.some(
      (sample) =>
        sample.phaseSecond >= 9 &&
        sample.streams.some((stream) => stream.watching && stream.frames > 30),
    ),
    "AV1 rewatch did not resume.",
  );
  const result = {
    codec: "av1",
    hardware: true,
    accepted: published[0].accepted,
    presented: Math.max(
      ...final.samples.flatMap((sample) =>
        sample.streams.map((stream) => stream.frames),
      ),
    ),
    stopWatchingBytes: stoppedBytes,
  };
  writeFileSync(
    join(root, ".local/macos/av1-results.json"),
    JSON.stringify(result, null, 2),
  );
  console.log(JSON.stringify(result));
} finally {
  watching.child.kill();
  publishing.child.kill();
}
