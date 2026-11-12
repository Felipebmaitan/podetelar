// Real Velopack packages/helper, local file feed, test publisher substitute.
// This does not qualify production media cleanup or fresh-machine installation.
import { execFileSync, spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
const root = resolve(import.meta.dirname, "..");
const output = join(root, ".local", `update-controls-${Date.now()}`);
const feed = join(output, "feed");
const stage = join(output, "stage");
const unpacked = join(output, "app");
mkdirSync(feed, { recursive: true });
mkdirSync(stage);
const env = {
  ...process.env,
  FREEDOMSTREAM_UPDATE_FEED: feed,
  CARGO_ENCODED_RUSTFLAGS:
    "-C\x1ftarget-feature=+crt-static\x1f-C\x1flink-arg=/DEPENDENTLOADFLAG:0x800",
};
const run = (file, args, extra = {}) =>
  execFileSync(file, args, {
    cwd: root,
    env,
    windowsHide: true,
    stdio: "inherit",
    ...extra,
  });
run("cargo", [
  "build",
  "--locked",
  "--release",
  "--manifest-path",
  "apps/installed-launcher/Cargo.toml",
  "--target",
  "x86_64-pc-windows-msvc",
  "--features",
  "update-qualification",
]);
const binaries = join(
  root,
  "apps/installed-launcher/target/x86_64-pc-windows-msvc/release",
);
copyFileSync(
  join(binaries, "freedomstream-installed.exe"),
  join(stage, "freedomstream-installed.exe"),
);
copyFileSync(
  join(binaries, "update-fixture.exe"),
  join(stage, "FreedomStream.exe"),
);
run("dotnet", ["tool", "restore"]);
for (const version of ["0.1.0-preview.1", "0.1.0-preview.2"]) {
  const destination = version.endsWith(".1") ? join(output, "old") : feed;
  run("dotnet", [
    "tool",
    "run",
    "vpk",
    "--",
    "pack",
    "--runtime",
    "win-x64",
    "--packId",
    "FreedomStreamUpdateQualification",
    "--packVersion",
    version,
    "--packDir",
    stage,
    "--mainExe",
    "freedomstream-installed.exe",
    "--channel",
    "preview",
    "--outputDir",
    destination,
  ]);
}
run(
  "powershell.exe",
  [
    "-NoProfile",
    "-Command",
    "Add-Type -AssemblyName System.IO.Compression.FileSystem; [IO.Compression.ZipFile]::ExtractToDirectory($env:UPDATE_FIXTURE_ZIP, $env:UPDATE_FIXTURE_DEST)",
  ],
  {
    env: {
      ...env,
      UPDATE_FIXTURE_ZIP: join(
        output,
        "old/FreedomStreamUpdateQualification-preview-Portable.zip",
      ),
      UPDATE_FIXTURE_DEST: unpacked,
    },
  },
);
const events = join(output, "events.txt");
async function launch(mode) {
  const child = spawn(
    join(unpacked, "current/freedomstream-installed.exe"),
    [],
    {
      windowsHide: true,
      stdio: "ignore",
      env: {
        ...process.env,
        UPDATE_FIXTURE_MODE: mode,
        UPDATE_FIXTURE_EVENTS: events,
      },
    },
  );
  const exit = new Promise((resolveExit, reject) => {
    child.on("error", reject);
    child.on("exit", (code) =>
      code === 0 ? resolveExit() : reject(new Error(`Fixture exited ${code}`)),
    );
  });
  let timer;
  try {
    await Promise.race([
      exit,
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          child.kill();
          reject(new Error("Fixture timeout"));
        }, 45000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
await launch("later");
if (
  !readFileSync(join(unpacked, "current/sq.version"), "utf8").includes(
    "0.1.0-preview.1",
  )
)
  throw new Error("Later installed an update");
await launch("restart");
const deadline = Date.now() + 45000;
while (
  !existsSync(events) ||
  !readFileSync(events, "utf8").includes("updated")
) {
  if (Date.now() > deadline)
    throw new Error("Updater did not restart version two");
  await new Promise((resolveWait) => setTimeout(resolveWait, 200));
}
console.log(
  `Passed real package download/Later/apply/restart with fixture publisher. Evidence: ${output}`,
);
