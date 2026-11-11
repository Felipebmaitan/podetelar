import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { installerInput } from "./installer-input.mjs";
import { installerNotices } from "./installer-notices.mjs";

const root = resolve(import.meta.dirname, "..");
const run = (file, args, options = {}) =>
  execFileSync(file, args, { cwd: root, windowsHide: true, ...options });
if (process.platform !== "win32")
  throw new Error("Build installers on Windows.");
if (run("git", ["status", "--porcelain"], { encoding: "utf8" }).trim())
  throw new Error("Commit the source before building an installer.");
const commit = run("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim();
const { executable, manifest } = installerInput(
  join(root, ".local/distribution"),
  commit,
);
const config = JSON.parse(
  readFileSync(join(root, "deploy/windows/installer.json")),
);
// CI run numbers make distinct preview builds monotonic without editing source.
const build = process.env.INSTALLER_BUILD_NUMBER;
if (!/^[1-9]\d{0,8}$/.test(build ?? ""))
  throw new Error(
    "Set INSTALLER_BUILD_NUMBER to a positive, increasing integer.",
  );
const version = `${manifest.version}-preview.${build}`;
const output = join(root, ".local/installers", version);
if (existsSync(output)) throw new Error("Installer output already exists.");
const stage = join(root, ".local/installer-stage", version);
if (existsSync(stage)) throw new Error("Installer staging already exists.");
mkdirSync(stage, { recursive: true });
copyFileSync(executable, join(stage, "FreedomStream.exe"));
copyFileSync(join(root, "LICENSE"), join(stage, "FreedomStream-MIT.txt"));
run(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "--manifest-path",
    "apps/installed-launcher/Cargo.toml",
    "--target",
    "x86_64-pc-windows-msvc",
  ],
  {
    stdio: "inherit",
    env: {
      ...process.env,
      CARGO_ENCODED_RUSTFLAGS:
        "-C\x1ftarget-feature=+crt-static\x1f-C\x1flink-arg=/DEPENDENTLOADFLAG:0x800",
    },
  },
);
copyFileSync(
  join(
    root,
    "apps/installed-launcher/target/x86_64-pc-windows-msvc/release",
    config.mainExe,
  ),
  join(stage, config.mainExe),
);
copyFileSync(
  join(root, "deploy/windows/Velopack-LICENSE.txt"),
  join(stage, "Velopack-LICENSE.txt"),
);
installerNotices(
  JSON.parse(
    run(
      "cargo",
      [
        "metadata",
        "--locked",
        "--format-version=1",
        "--filter-platform=x86_64-pc-windows-msvc",
        "--manifest-path",
        "apps/installed-launcher/Cargo.toml",
      ],
      { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
    ),
  ),
  stage,
);
run("dotnet", ["tool", "restore"], { stdio: "inherit" });
run(
  "dotnet",
  [
    "tool",
    "run",
    "vpk",
    "--",
    "pack",
    "--runtime",
    "win-x64",
    "--packId",
    config.packageId,
    "--packVersion",
    version,
    "--packDir",
    stage,
    "--mainExe",
    config.mainExe,
    "--packTitle",
    config.title,
    "--channel",
    "preview",
    "--icon",
    join(root, "assets/windows/freedomstream.ico"),
    "--outputDir",
    output,
  ],
  { stdio: "inherit" },
);
console.log(`Unsigned preview installer: ${output}. No release was published.`);
