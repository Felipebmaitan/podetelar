import { execFileSync as runFile } from "node:child_process";
const execFileSync = (file, args, options = {}) =>
  runFile(file, args, { ...options, windowsHide: true });
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  utimesSync,
} from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import { preparePortableRuntime } from "./portable-runtime.mjs";

if (process.platform !== "win32")
  throw new Error("Build the Windows release on Windows.");
const root = resolve(import.meta.dirname, "..");
const cargoTarget = resolve(root, process.env.CARGO_TARGET_DIR ?? "target");
const commit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: root,
  encoding: "utf8",
}).trim();
if (!/^[a-f0-9]{40}$/.test(commit))
  throw new Error("A committed source checkout is required.");
if (
  execFileSync("git", ["status", "--porcelain"], {
    cwd: root,
    encoding: "utf8",
  }).trim() &&
  !process.argv.includes("--qualification")
)
  throw new Error(
    "Commit the source before packaging a distributable release. --qualification permits an explicitly local test artifact.",
  );
const version = JSON.parse(readFileSync(join(root, "package.json"))).version;
const id = `${version}-${commit.slice(0, 12)}`;
// Always reconcile the executable with this checkout before attaching its commit.
execFileSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    join(root, "scripts/native.ps1"),
    "build",
  ],
  { cwd: root, stdio: "inherit" },
);
const output = resolve(
  root,
  ".local",
  "distribution",
  ...(process.argv.includes("--qualification")
    ? [`qualification-${Date.now()}`]
    : []),
  id,
);
if (existsSync(output))
  throw new Error("The immutable package directory already exists.");
mkdirSync(output, { recursive: true });
const notices = join(output, "notices");
mkdirSync(notices);
const decoderPatch = join(notices, "source-patches", "native-decoder");
mkdirSync(decoderPatch, { recursive: true });
for (const file of [
  "apps/publisher/decoder/freedomstream_decoder.h",
  "apps/publisher/decoder/deferred_i420.h",
  "apps/publisher/decoder/bounded_readback.h",
  "apps/publisher/decoder/native_output_guard.h",
  "apps/publisher/decoder/native_output_mixer.h",
  "scripts/patch-native-output.mjs",
  "scripts/setup-native-audio.mjs",
  "scripts/setup-native-sdk.mjs",
  "deploy/native-sdk.json",
  "deploy/native-audio.json",
  "scripts/setup-native-decoder.mjs",
  "deploy/native-decoder.json",
]) {
  copyFileSync(join(root, file), join(decoderPatch, basename(file)));
}
writeFileSync(
  join(decoderPatch, "README.txt"),
  "PodeTelar modifies webrtc-sys 0.3.53 (Apache-2.0) for native Direct3D11 decoders through dynamically linked GStreamer, expired weak track pruning, output-only Windows audio, remote track gain and output delay qualification. The five adapter/helper headers and patch script are MIT licensed. The included headers, setup/patch scripts and checksum pins reproduce the adapters from the original crates. libwebrtc 0.4.0 exposes a typed audio handle; LiveKit 0.10.1 adds read-only negotiated stream association accessors for qualification. Existing WebRTC runtime and third-party licenses still apply. No browser runtime is included.\n",
);
const { dumpbin } = await preparePortableRuntime(root, output, notices);
const sha256 = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
// The launcher must start even when the target PC has no shared Visual C++ CRT.
execFileSync(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "-p",
    "freedomstream-portable",
    "--target",
    "x86_64-pc-windows-msvc",
    "--target-dir",
    join(root, ".local/portable-target"),
  ],
  {
    cwd: root,
    stdio: "inherit",
    env: {
      ...process.env,
      RUSTFLAGS:
        "-C target-feature=+crt-static -C link-arg=/DEPENDENTLOADFLAG:0x800",
      CARGO_ENCODED_RUSTFLAGS:
        "-C\x1ftarget-feature=+crt-static\x1f-C\x1flink-arg=/DEPENDENTLOADFLAG:0x800",
    },
  },
);
const launcher = join(
  root,
  ".local/portable-target/x86_64-pc-windows-msvc/release/freedomstream-portable.exe",
);
if (
  /^\s+(?:MSVCP|VCRUNTIME|api-ms-win-crt-).*\.dll\s*$/im.test(
    execFileSync(dumpbin, ["/DEPENDENTS", launcher], { encoding: "utf8" }),
  )
)
  throw new Error("The portable launcher requires a shared CRT.");
if (
  !/^\s+0*800\s+Dependent Load Flags?\s*$/im.test(
    execFileSync(dumpbin, ["/LOADCONFIG", launcher], { encoding: "utf8" }),
  )
)
  throw new Error(
    "The launcher must resolve its Windows imports from System32.",
  );
copyFileSync(join(root, "LICENSE"), join(notices, "FreedomStream-MIT.txt"));
copyFileSync(
  join(root, "assets/fonts/OFL-Inter.txt"),
  join(notices, "Inter-OFL.txt"),
);
const metadata = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--locked",
      "--format-version=1",
      "--filter-platform=x86_64-pc-windows-msvc",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  ),
);
const included = new Set(metadata.resolve.nodes.map((node) => node.id));
const index = [
  "# Windows publisher dependency notices",
  "",
  "PodeTelar is MIT licensed. Each dependency retains its own license. The curated runtime, complete corresponding library sources and build recipes accompany this executable. See RUNTIME.md and sources/README.md. No codec patent license or hardware qualification is implied.",
  "",
  "Inter Regular and SemiBold fonts: SIL Open Font License 1.1. Copyright 2020 The Inter Project Authors. See Inter-OFL.txt.",
  "",
];
for (const pkg of metadata.packages
  .filter((pkg) => included.has(pkg.id))
  .sort((a, b) => a.id.localeCompare(b.id))) {
  if (["freedomstream-publisher", "freedomstream-portable"].includes(pkg.name))
    continue;
  const directory = resolve(pkg.manifest_path, "..");
  const files = readdirSync(directory, {
    recursive: true,
    withFileTypes: true,
  }).filter(
    (entry) =>
      entry.isFile() &&
      /^(?:licen[cs]e|copying|notice|ofl)(?:[._-]|$)/i.test(entry.name),
  );
  if (
    pkg.license_file &&
    !files.some(
      (file) =>
        join(file.parentPath, file.name) ===
        resolve(directory, pkg.license_file),
    )
  )
    files.push({
      name: basename(pkg.license_file),
      parentPath: resolve(directory, pkg.license_file, ".."),
    });
  index.push(
    `## ${pkg.name} ${pkg.version}`,
    "",
    `License: ${pkg.license ?? "see included license file"}. Source: ${pkg.repository ?? `https://crates.io/crates/${pkg.name}/${pkg.version}`}`,
    "",
  );
  if (!files.length) {
    const overrides = JSON.parse(
      readFileSync(join(root, "deploy/license-overrides.json")),
    )[`${pkg.name}@${pkg.version}`];
    if (!overrides?.length)
      throw new Error(
        `Missing license text for ${pkg.name}. Qualify it before distributing.`,
      );
    const target = join(notices, "cargo", `${pkg.name}-${pkg.version}`);
    mkdirSync(target, { recursive: true });
    for (const [i, override] of overrides.entries()) {
      const source = join(root, override.file);
      if (sha256(source) !== override.sha256)
        throw new Error(`Tampered license override for ${pkg.name}.`);
      copyFileSync(source, join(target, `${i}-${basename(source)}`));
      index.push(
        `License provenance: ${override.source}${override.selectedLicense ? `; ${override.selectedLicense}` : ""}`,
        "",
      );
    }
    if (pkg.name === "livekit")
      index.push(
        "Local SDK change: advertise TF_STEREO for two-channel AddTrackRequest. See scripts/setup-native-sdk.mjs and deploy/native-sdk.json for the pinned original and patch.",
        "",
      );
    continue;
  }
  const target = join(notices, "cargo", `${pkg.name}-${pkg.version}`);
  for (const file of files) {
    const source = join(file.parentPath, file.name);
    // Source-tree paths remain relative; never copy unrelated build/cache data.
    const destination = join(target, relative(directory, source));
    mkdirSync(resolve(destination, ".."), { recursive: true });
    copyFileSync(source, destination);
  }
}
const buildRoot = join(cargoTarget, "release/build");
const webrtc = readdirSync(buildRoot, {
  recursive: true,
  withFileTypes: true,
}).filter(
  (entry) =>
    entry.isFile() &&
    entry.name === "LICENSE.md" &&
    entry.parentPath.includes("win-x64-release-webrtc-"),
);
if (!webrtc.length)
  throw new Error("Missing prebuilt WebRTC third-party license text.");
copyFileSync(
  join(webrtc[0].parentPath, webrtc[0].name),
  join(notices, "WebRTC-third-party-LICENSE.md"),
);
writeFileSync(join(notices, "README.md"), index.join("\n"));
const files = {};
for (const entry of readdirSync(output, {
  recursive: true,
  withFileTypes: true,
}).filter((entry) => entry.isFile())) {
  const path = join(entry.parentPath, entry.name);
  files[relative(output, path).replaceAll("\\", "/")] = sha256(path);
  utimesSync(
    path,
    new Date("2000-01-01T00:00:00Z"),
    new Date("2000-01-01T00:00:00Z"),
  );
}
writeFileSync(
  join(output, "release.json"),
  JSON.stringify(
    {
      id,
      version,
      commit,
      qualificationOnly: process.argv.includes("--qualification"),
      files,
    },
    null,
    2,
  ) + "\n",
);
// Quote a literal path, not a command assembled from user-supplied values.
const quote = (value) => "'" + value.replaceAll("'", "''") + "'";
const archive = join(resolve(output, ".."), `freedomstream-payload-${id}.zip`);
execFileSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-Command",
    `Compress-Archive -Path ${quote(join(output, "*"))} -DestinationPath ${quote(archive)} -CompressionLevel Optimal`,
  ],
  { stdio: "inherit" },
);
const payload = readFileSync(archive);
const footer = Buffer.alloc(56);
footer.writeBigUInt64LE(BigInt(payload.length));
createHash("sha256").update(payload).digest().copy(footer, 8);
footer.write("FREEDOMSTREAM01!", 40, "ascii");
const executable = join(
  resolve(output, ".."),
  `freedomstream-windows-x64-${id}.exe`,
);
writeFileSync(
  executable,
  Buffer.concat([readFileSync(launcher), payload, footer]),
);
const { size } = await import("node:fs").then((fs) => fs.statSync(executable));
writeFileSync(
  join(resolve(output, ".."), "publisher.json"),
  JSON.stringify(
    {
      filename: basename(executable),
      version,
      commit,
      sha256: sha256(executable),
      size,
    },
    null,
    2,
  ) + "\n",
);
console.log(
  `Packaged ${id}${process.argv.includes("--qualification") ? " (local qualification only)" : ""}. Portable EXE and publisher.json: ${resolve(output, "..")}`,
);
