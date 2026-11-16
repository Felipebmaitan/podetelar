// An owned, real GUI window and stereo CoreAudio source. No microphones/mocks.
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { root } from "./macos-runtime.mjs";
if (process.platform !== "darwin") throw new Error("Run on macOS.");
const app = join(root, ".local/macos/MediaQualification.app");
mkdirSync(join(app, "Contents/MacOS"), { recursive: true });
mkdirSync(join(app, "Contents/Resources"), { recursive: true });
writeFileSync(
  join(app, "Contents/Info.plist"),
  `<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.podetelar.qualification</string><key>CFBundleName</key><string>PodeTelar qualification</string><key>CFBundleExecutable</key><string>MediaFixture</string><key>CFBundlePackageType</key><string>APPL</string><key>LSMinimumSystemVersion</key><string>14.0</string></dict></plist>`,
);
const executable = join(app, "Contents/MacOS/MediaFixture");
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
execFileSync("codesign", ["--force", "--sign", "-", app], { stdio: "inherit" });
const fixture = spawn(executable, [], { stdio: ["ignore", "pipe", "ignore"] });
try {
  await new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("Media fixture did not start.")),
      5000,
    );
    fixture.once("exit", () => {
      clearTimeout(timer);
      reject(new Error("Media fixture stopped."));
    });
    fixture.stdout.once("data", () => {
      clearTimeout(timer);
      resolve();
    });
  });
  // "ready" precedes AppKit's opening animation. Select this process's real
  // window only after ScreenCaptureKit reports stable geometry, not a transient
  // animation surface or a recently closed fixture with the same title.
  const bundle =
    process.env.MACOS_TEST_APP ||
    JSON.parse(readFileSync(join(root, ".local/macos/latest-release.json")))
      .app;
  const helper = join(bundle, "Contents/MacOS/PodeTelar");
  const deadline = Date.now() + 10000;
  let previous = "",
    stableSince = Date.now(),
    sourceId;
  while (Date.now() < deadline) {
    const items = JSON.parse(
      execFileSync(helper, ["enumerate"], {
        input: JSON.stringify({ action: "enumerate" }),
        encoding: "utf8",
        timeout: 2500,
      }),
    );
    const window = items.find(
      (item) =>
        item.kind === "window" &&
        item.pid === fixture.pid &&
        item.width > 144 &&
        item.height > 128,
    );
    const key = window
      ? JSON.stringify([window.id, window.width, window.height])
      : "";
    if (!key || key !== previous) {
      previous = key;
      stableSince = Date.now();
    } else if (Date.now() - stableSince >= 500) {
      sourceId = `window:${window.id}`;
      break;
    }
    await delay(150);
  }
  if (!sourceId)
    throw new Error(
      "Fixture window did not become stable in ScreenCaptureKit.",
    );
  await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["scripts/qualify-macos-media.mjs"], {
      cwd: root,
      env: {
        ...process.env,
        MACOS_TEST_SOURCE_KIND: "window",
        MACOS_TEST_SOURCE_LABEL: "PodeTelar qualification",
        MACOS_TEST_SOURCE_ID: sourceId,
        MACOS_TEST_AUDIO: "1",
      },
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0
        ? resolve()
        : reject(new Error("macOS window/audio qualification failed.")),
    );
  });
} finally {
  fixture.kill();
}
