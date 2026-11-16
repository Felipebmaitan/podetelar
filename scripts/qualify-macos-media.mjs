// Real Apple hardware → local SFU → native VideoToolbox decoder, no invitation logs.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { root, macosEnvironment } from "./macos-runtime.mjs";
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("Use Apple Silicon macOS.");
const latest = join(root, ".local/macos/latest-release.json");
const app =
  process.env.MACOS_TEST_APP ??
  (existsSync(latest) ? JSON.parse(readFileSync(latest)).app : null);
const executable = app
  ? join(app, "Contents/Helpers/PodeTelar.app/Contents/MacOS/FreedomStream")
  : join(root, "target/release/freedomstream-publisher");
const env = app
  ? {
      ...process.env,
      DYLD_LIBRARY_PATH: "",
      DYLD_FALLBACK_LIBRARY_PATH: "",
      GST_PLUGIN_SYSTEM_PATH_1_0: join(app, "Contents/PlugIns"),
      GST_PLUGIN_PATH_1_0: "",
      GST_PLUGIN_SCANNER_1_0: join(app, "Contents/MacOS/gst-plugin-scanner"),
      GST_REGISTRY_1_0: join(root, ".local/macos/qualification-registry.bin"),
    }
  : macosEnvironment();
const origin = process.env.MACOS_TEST_ORIGIN ?? "http://localhost:3000";
function native(option, request) {
  const child = spawn(executable, [option], {
    env: { ...env, FREEDOMSTREAM_DIAGNOSTICS: "1" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let output = "",
    error = "";
  child.stdout.on("data", (chunk) => {
    output += chunk;
    if (output.length > 8 * 1024 * 1024) child.kill();
  });
  child.stderr.on("data", (chunk) => {
    if (error.length < 2 * 1024 * 1024) error += chunk;
  });
  if (request) child.stdin.end(JSON.stringify(request));
  else child.stdin.end();
  const done = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("Native macOS qualification timed out."));
    }, 60_000);
    child.on("error", () => {
      clearTimeout(timer);
      reject(new Error("Native macOS app could not launch."));
    });
    child.on("exit", (code) => {
      clearTimeout(timer);
      if (option === "--watch-fixture")
        writeFileSync(
          join(
            root,
            `.local/macos/decoder-${request.hevcOnly ? "h265" : "h264"}.log`,
          ),
          error
            .split(/\r?\n/)
            .filter(
              (line) =>
                line.includes("vtdec") ||
                line.startsWith("macOS decoder timing:"),
            )
            .join("\n"),
        );
      if (code !== 0) {
        writeFileSync(
          join(root, ".local/macos/media-diagnostics.txt"),
          error
            .split(/\r?\n/)
            .filter((line) => line.startsWith("PodeTelar diagnostic:"))
            .join("\n"),
        );
        reject(
          new Error(
            `Native macOS ${option} failed (${code}): ${error.split(/\r?\n/).find((line) => line.startsWith("PodeTelar:")) ?? "check capture permission and the local SFU"}`,
          ),
        );
        return;
      }
      try {
        resolve(
          output
            .split(/\r?\n/)
            .filter((line) => line.startsWith("{") || line.startsWith("["))
            .map((line) => JSON.parse(line)),
        );
      } catch {
        reject(new Error("Invalid native qualification output."));
      }
    });
  });
  return { child, done };
}
const sources = (await native("--sources").done)[0];
const source = sources.find(
  (source) =>
    (!process.env.MACOS_TEST_SOURCE_LABEL ||
      source.label.includes(process.env.MACOS_TEST_SOURCE_LABEL)) &&
    source.id.startsWith(
      process.env.MACOS_TEST_SOURCE_KIND === "window" ? "window:" : "monitor:",
    ),
);
assert.ok(
  source,
  "ScreenCaptureKit did not return a capture source; allow screen recording first.",
);
const results = [];
for (const codec of process.env.MACOS_TEST_CODEC
  ? [process.env.MACOS_TEST_CODEC]
  : ["h264", "h265"]) {
  const response = await fetch(`${origin}/api/rooms`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Origin: origin,
      Authorization: `Bearer ${process.env.ADMIN_TOKEN}`,
    },
    body: JSON.stringify({
      name: "macOS hardware qualification",
      displayName: "Test owner",
    }),
    signal: AbortSignal.timeout(5000),
  });
  assert.equal(response.status, 201, "Could not create an isolated test room.");
  const room = await response.json();
  const invitationUrl = room.invitationUrl;
  assert.equal(typeof invitationUrl, "string");
  const watcher = native("--watch-fixture", {
    invitationUrl,
    seconds: 18,
    h264Only: codec === "h264",
    hevcOnly: codec === "h265",
    consumePictures: true,
    diagnostics: true,
  });
  const publisher = native("--publish-fixture", {
    invitationUrl,
    displayName: "Mac hardware publisher",
    codec,
    seconds: 18,
    sourceId: source.id,
    audio: { mode: "off" },
    sourceAudio: process.env.MACOS_TEST_AUDIO === "1",
    quality: {
      resolution: "p720",
      fps: 30,
      profile: "motion",
      bitrate: { mode: "auto" },
    },
  });
  let viewed, published;
  try {
    [viewed, published] = await Promise.all([watcher.done, publisher.done]);
  } finally {
    watcher.child.kill();
    publisher.child.kill();
  }
  writeFileSync(
    join(root, `.local/macos/watch-${codec}.json`),
    JSON.stringify(viewed, null, 2),
  );
  writeFileSync(
    join(root, `.local/macos/publish-${codec}.json`),
    JSON.stringify(published, null, 2),
  );
  const final = published.find((value) => value.metrics);
  assert.ok(
    final?.metrics.accepted > 100,
    "Hardware publication did not deliver frames.",
  );
  const watch = viewed.find((value) => value.closed);
  assert.ok(
    watch?.samples.some((sample) =>
      sample.decoders.some((decoder) => decoder.hardware),
    ),
    "Native WebRTC did not report hardware decoding.",
  );
  assert.ok(
    watch.samples.some((sample) =>
      sample.streams.some((stream) => stream.frames > 0 || stream.pictures > 0),
    ),
    "No native pictures were presented.",
  );
  const stopped = watch.samples.filter(
    (sample) => sample.phaseSecond >= 6 && sample.phaseSecond < 8,
  );
  assert.ok(stopped.length >= 2, "Stop watching window was not observed.");
  assert.ok(
    stopped.at(-1).receivedBytes - stopped[0].receivedBytes < 32_000,
    "Stop watching retained incoming media traffic.",
  );
  assert.ok(
    watch.samples.some(
      (sample) =>
        sample.phaseSecond >= 9 &&
        sample.streams.some((stream) => stream.watching),
    ),
    "Rewatch did not resume.",
  );
  if (process.env.MACOS_TEST_AUDIO === "1") {
    assert.ok(
      watch.samples.some((sample) =>
        sample.streams.some(
          (stream) =>
            stream.audio.frames > 48000 &&
            stream.audio.peak > 0.001 &&
            stream.audio.channel_difference_peak > 0.001,
        ),
      ),
      "Native stereo audio was not received.",
    );
    assert.ok(
      watch.samples.some(
        (sample) =>
          sample.nativeAudio.rendering &&
          sample.nativeAudio.platformOutputStarts > 0,
      ),
      "CoreAudio output did not start.",
    );
    assert.ok(
      watch.samples.every(
        (sample) =>
          !sample.nativeAudio.recordingEnabled &&
          !sample.nativeAudio.recordingInitialized,
      ),
      "WebRTC opened an audio input.",
    );
  }
  results.push({
    codec,
    encoder: final.encoder,
    frames: final.metrics.accepted,
    previewCaptures: final.previewCaptures,
    decoders: [
      ...new Set(
        watch.samples.flatMap((sample) =>
          sample.decoders
            .filter((decoder) => decoder.hardware)
            .map((decoder) => decoder.implementation),
        ),
      ),
    ],
    audio:
      process.env.MACOS_TEST_AUDIO === "1"
        ? {
            receivedFrames: Math.max(
              ...watch.samples.flatMap((sample) =>
                sample.streams.map((stream) => stream.audio.frames),
              ),
            ),
            outputStarted: watch.samples.some(
              (sample) => sample.nativeAudio.rendering,
            ),
          }
        : null,
    stopWatchingBytes: stopped.at(-1).receivedBytes - stopped[0].receivedBytes,
  });
}
writeFileSync(
  join(root, ".local/macos/media-results.json"),
  JSON.stringify({ sourceKind: source.id.split(":")[0], results }, null, 2) +
    "\n",
);
console.log(JSON.stringify({ sourceKind: source.id.split(":")[0], results }));
