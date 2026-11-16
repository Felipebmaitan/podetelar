// An owned, real GUI window and stereo CoreAudio source. No microphones/mocks.
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
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
  await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["scripts/qualify-macos-media.mjs"], {
      cwd: root,
      env: {
        ...process.env,
        MACOS_TEST_SOURCE_KIND: "window",
        MACOS_TEST_SOURCE_LABEL: "PodeTelar qualification",
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
