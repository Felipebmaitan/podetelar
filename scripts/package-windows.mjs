import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  utimesSync,
} from "node:fs";
import { basename, join, relative, resolve } from "node:path";

if (process.platform !== "win32")
  throw new Error("Build the Windows release on Windows.");
const root = resolve(import.meta.dirname, "..");
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
cpSync(
  join(root, ".local/gstreamer/share/licenses"),
  join(notices, "gstreamer"),
  { recursive: true },
);
copyFileSync(
  join(root, ".local/gstreamer/share/versions.txt"),
  join(notices, "gstreamer-versions.txt"),
);
const sha256 = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
copyFileSync(
  join(root, "target/release/freedomstream-publisher.exe"),
  join(output, "freedomstream-publisher.exe"),
);
for (const file of readdirSync(join(root, "deploy/windows")))
  copyFileSync(join(root, "deploy/windows", file), join(output, file));
copyFileSync(
  join(root, "deploy/native-runtime.json"),
  join(output, "native-runtime.json"),
);
copyFileSync(join(root, "LICENSE"), join(notices, "FreedomStream-MIT.txt"));
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
  "FreedomStream is MIT licensed. Each dependency retains the license listed below. The runtime is downloaded directly from GStreamer; its complete share/licenses directory remains installed. No codec patent license or hardware qualification is implied.",
  "",
];
for (const pkg of metadata.packages
  .filter((pkg) => included.has(pkg.id))
  .sort((a, b) => a.id.localeCompare(b.id))) {
  if (pkg.name === "freedomstream-publisher") continue; // root MIT notice above
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
const buildRoot = join(root, "target/release/build");
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
const archive = join(
  resolve(output, ".."),
  `freedomstream-windows-x64-${id}.zip`,
);
execFileSync(
  "powershell.exe",
  [
    "-NoProfile",
    "-Command",
    `Compress-Archive -Path ${quote(join(output, "*"))} -DestinationPath ${quote(archive)} -CompressionLevel Optimal`,
  ],
  { stdio: "inherit" },
);
const { size } = await import("node:fs").then((fs) => fs.statSync(archive));
writeFileSync(
  join(resolve(output, ".."), "publisher.json"),
  JSON.stringify(
    {
      filename: basename(archive),
      version,
      commit,
      sha256: sha256(archive),
      size,
    },
    null,
    2,
  ) + "\n",
);
console.log(
  `Packaged ${id}${process.argv.includes("--qualification") ? " (local qualification only)" : ""}. Archive and publisher.json: ${resolve(output, "..")}`,
);
