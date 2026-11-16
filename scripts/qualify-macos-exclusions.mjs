// Real late-launch/restart audio sources; no microphone or synthetic capture.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { setTimeout as delay } from "node:timers/promises";
import { root, macosEnvironment } from "./macos-runtime.mjs";
if (process.platform !== "darwin") throw new Error("Run on macOS.");
const directory = join(tmpdir(), `podetelar-audio-${Date.now()}`);
const fixtures = [];
for (const kind of ["allowed", "excluded"]) {
  const app = join(directory, `${kind}.app`);
  mkdirSync(join(app, "Contents/MacOS"), { recursive: true });
  writeFileSync(
    join(app, "Contents/Info.plist"),
    `<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.podetelar.${kind}-qualification</string><key>CFBundleExecutable</key><string>Fixture</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>`,
  );
  const executable = join(app, "Contents/MacOS/Fixture");
  execFileSync(
    "xcrun",
    [
      "swiftc",
      "-swift-version",
      "5",
      "-warnings-as-errors",
      "-O",
      "-target",
      "arm64-apple-macos14.0",
      "apps/macos/MediaFixture.swift",
      "-o",
      executable,
    ],
    { cwd: root, stdio: "inherit" },
  );
  execFileSync("codesign", ["--force", "--sign", "-", app], {
    stdio: "inherit",
  });
  fixtures.push(executable);
}
const helper = spawn(
  join(root, ".local/macos/PodeTelar.app/Contents/MacOS/PodeTelar"),
  ["audio"],
  { stdio: ["pipe", "pipe", "ignore"] },
);
helper.stdin.end(
  JSON.stringify({
    action: "audio",
    exclusions: ["org.podetelar.excluded-qualification"],
  }),
);
let pending = Buffer.alloc(0),
  sampleIndex = 0;
const started = Date.now(),
  bins = new Map();
// Exercise Rust's no-sample handling while the desktop starts quiet for >3s.
const native = spawn(
  join(root, "target/release/freedomstream-publisher"),
  ["--qualify-audio"],
  { env: macosEnvironment(), stdio: ["pipe", "pipe", "ignore"] },
);
let nativeOutput = "";
native.stdout.on("data", (chunk) => {
  nativeOutput += chunk;
  assert.ok(nativeOutput.length < 65536);
});
native.stdin.end(
  JSON.stringify({
    choice: { mode: "desktop" },
    exclusions: ["org.podetelar.excluded-qualification"],
    seconds: 14,
    playback_tone: false,
  }),
);
const nativeDone = new Promise((resolve) => native.on("exit", resolve));
helper.stdout.on("data", (chunk) => {
  pending = Buffer.concat([pending, chunk]);
  while (pending.length >= 4) {
    const length = pending.readUInt32LE(0);
    assert.ok(length <= 192000 && length % 4 === 0);
    if (pending.length < length + 4) break;
    const second = Math.floor((Date.now() - started) / 1000);
    const bin = bins.get(second) ?? { count: 0, sums: [0, 0, 0, 0] };
    for (let offset = 4; offset < length + 4; offset += 4) {
      const value = pending.readInt16LE(offset) / 32768;
      for (const [i, frequency] of [440, 1320].entries()) {
        const phase = (sampleIndex * 2 * Math.PI * frequency) / 48000;
        bin.sums[i * 2] += value * Math.cos(phase);
        bin.sums[i * 2 + 1] += value * Math.sin(phase);
      }
      sampleIndex++;
      bin.count++;
    }
    bins.set(second, bin);
    pending = pending.subarray(length + 4);
  }
});
const children = [];
function launch(index) {
  const child = spawn(fixtures[index], [], {
    env: {
      ...process.env,
      PODETELAR_TEST_FREQUENCY: index ? "1320" : "440",
      PODETELAR_TEST_VOLUME: index ? "0.02" : "0.005",
    },
    stdio: "ignore",
  });
  children.push(child);
  return child;
}
try {
  await delay(4500);
  launch(0);
  await delay(2500);
  const excluded = launch(1);
  await delay(2000);
  excluded.kill();
  await delay(500);
  launch(1);
  await delay(3500);
  assert.equal(helper.exitCode, null, "Capture helper stopped unexpectedly.");
  const measurements = [...bins]
    .filter(([second, bin]) => second >= 6 && bin.count > 24000)
    .map(([second, bin]) => ({
      second,
      allowed: (2 * Math.hypot(bin.sums[0], bin.sums[1])) / bin.count,
      excluded: (2 * Math.hypot(bin.sums[2], bin.sums[3])) / bin.count,
    }));
  assert.ok(measurements.length >= 5, "Insufficient actual captured audio.");
  assert.ok(
    measurements.every((value) => value.allowed > 0.003),
    "Late allowed application was not captured.",
  );
  assert.ok(
    measurements.every((value) => value.excluded < 0.0001),
    "Excluded application leaked after launch/restart.",
  );
  assert.equal(
    await nativeDone,
    0,
    "Rust capture failed after initial quiet desktop.",
  );
  assert.ok(JSON.parse(nativeOutput).frames > 48000);
  writeFileSync(
    join(root, ".local/macos/late-audio-results.json"),
    JSON.stringify(measurements, null, 2),
  );
  console.log(
    "PASS: late allowed application captured; excluded application stayed inaudible across launch and restart.",
  );
} finally {
  helper.kill();
  native.kill();
  for (const child of children) child.kill();
}
