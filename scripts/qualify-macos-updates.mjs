// Actual macOS Velopack packages, download/Later/integrity and helper replacement.
// A process fixture stands in for media. This does not claim installed media shutdown.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { root, macosEnvironment, sha256 } from "./macos-runtime.mjs";
if (process.platform !== "darwin") throw new Error("Run on macOS.");
// Finder relaunch must not require Documents/Desktop privacy access merely to
// write fixture state. Production state lives in Application Support.
const output = mkdtempSync(join(tmpdir(), "podetelar-update-qualification-"));
const feed = join(output, "feed"),
  app = join(output, "stage/PodeTelar.app");
mkdirSync(join(app, "Contents/MacOS"), { recursive: true });
mkdirSync(join(app, "Contents/Resources"));
mkdirSync(feed);
const env = {
  ...macosEnvironment(),
  PODETELAR_UPDATE_FEED: feed,
  PODETELAR_MACOS_STATE_TEST: join(output, "state"),
};
const run = (file, args) =>
  execFileSync(file, args, { cwd: root, env, stdio: "inherit" });
run("cargo", [
  "build",
  "--locked",
  "--release",
  "--manifest-path",
  "apps/installed-launcher/Cargo.toml",
  "--features",
  "update-qualification",
]);
const binaries = join(root, "apps/installed-launcher/target/release");
copyFileSync(
  join(binaries, "podetelar-installed"),
  join(app, "Contents/MacOS/PodeTelarInstalled"),
);
copyFileSync(
  join(binaries, "update-fixture"),
  join(app, "Contents/MacOS/PodeTelar"),
);
writeFileSync(
  join(app, "Contents/Info.plist"),
  readFileSync(join(root, "deploy/macos/Info.plist"), "utf8")
    .replace(
      "<string>PodeTelar</string>\n<key>CFBundlePackageType",
      "<string>PodeTelarInstalled</string>\n<key>CFBundlePackageType",
    )
    .replace("app.podetelar.desktop", "app.podetelar.update-fixture"),
);
const id = `PodeTelarMacUpdate${Date.now()}`;
run("dotnet", ["tool", "restore"]);
for (const version of ["0.1.0-preview.1", "0.1.0-preview.2"]) {
  run("dotnet", [
    "tool",
    "run",
    "vpk",
    "--",
    "pack",
    "--runtime",
    "osx-arm64",
    "--packId",
    id,
    "--packTitle",
    "PodeTelar",
    "--packVersion",
    version,
    "--packDir",
    app,
    "--mainExe",
    "PodeTelarInstalled",
    "--channel",
    "osx-arm64",
    "--delta",
    "None",
    "--noInst",
    "--outputDir",
    version.endsWith(".1") ? join(output, "old") : feed,
  ]);
}
const old = join(output, "old"),
  unpacked = join(output, "installed");
mkdirSync(unpacked);
const portable = readdirSync(old).find((name) =>
  name.endsWith("-Portable.zip"),
);
assert.ok(portable, "Missing macOS portable bundle.");
run("ditto", ["-xk", join(old, portable), unpacked]);
const installed = join(unpacked, "PodeTelar.app/Contents/MacOS"),
  manifest = join(unpacked, "PodeTelar.app/Contents/Resources/sq.version");
const events = join(output, "events.txt");
async function launch(mode) {
  const child = spawn(join(installed, "PodeTelarInstalled"), [], {
    env: { ...env, UPDATE_FIXTURE_EVENTS: events, UPDATE_FIXTURE_MODE: mode },
    stdio: "ignore",
  });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("macOS update fixture timed out."));
    }, 45_000);
    child.on("error", reject);
    child.on("exit", (code) => {
      clearTimeout(timer);
      code === 0
        ? resolve()
        : reject(new Error(`macOS update fixture failed (${code}).`));
    });
  });
}
await launch("later");
assert.match(readFileSync(manifest, "utf8"), /0\.1\.0-preview\.1/);
assert.match(readFileSync(events, "utf8"), /available\nready\nlater/);
const metadata = JSON.parse(
  readFileSync(join(feed, "releases.osx-arm64.json")),
);
const full = metadata.Assets.find((asset) => asset.Type === "Full");
const cache = join(
  homedir(),
  "Library/Caches/velopack",
  id,
  "packages",
  full.FileName,
);
assert.equal(sha256(cache), full.SHA256.toLowerCase());
writeFileSync(cache, "corrupted cached package");
await launch("later");
assert.equal(
  sha256(cache),
  full.SHA256.toLowerCase(),
  "Corrupted cache was not replaced with verified bytes.",
);
assert.match(readFileSync(manifest, "utf8"), /0\.1\.0-preview\.1/);
await launch("restart");
const deadline = Date.now() + 45_000;
while (Date.now() < deadline) {
  if (
    existsSync(manifest) &&
    readFileSync(manifest, "utf8").includes("0.1.0-preview.2") &&
    readFileSync(events, "utf8").includes("updated")
  )
    break;
  await delay(250);
}
assert.match(readFileSync(manifest, "utf8"), /0\.1\.0-preview\.2/);
assert.match(readFileSync(events, "utf8"), /restart\nupdated/);
console.log(
  "PASS: real macOS update discovery, authenticated download, Later, corrupted-cache recovery, explicit restart, replacement and relaunch. Media teardown is a separate check.",
);
