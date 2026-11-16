import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import {
  root,
  runtime,
  pin,
  macosEnvironment,
  sha256,
} from "./macos-runtime.mjs";
import { macosRelease, releaseVersion } from "./macos-release.mjs";
import { macosNotices } from "./macos-notices.mjs";
import { installerUpdateFeed } from "./installer-feed.mjs";
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("Package on Apple Silicon macOS.");
// Build tools must use their own libraries. In particular Homebrew Git needs
// a newer libintl than GStreamer's SDK provides. Media binaries already carry
// the SDK rpath during development; the final probe uses bundle-relative paths.
const env = {
  ...macosEnvironment(),
  DYLD_LIBRARY_PATH: "",
  DYLD_FALLBACK_LIBRARY_PATH: "",
};
const run = (file, args, options = {}) =>
  execFileSync(file, args, { cwd: root, env, stdio: "inherit", ...options });
const output = (file, args) =>
  run(file, args, { stdio: "pipe", encoding: "utf8" }).trim();
const qualification = process.argv.includes("--qualification");
if (!qualification && output("git", ["status", "--porcelain"]))
  throw new Error(
    "Commit source before packaging; --qualification creates a local test artifact.",
  );
const commit = output("git", ["rev-parse", "HEAD"]);
const version = releaseVersion(
  JSON.parse(readFileSync(join(root, "package.json"))).version,
  process.env.RELEASE_TAG,
);
const build = process.env.INSTALLER_BUILD_NUMBER;
if (!/^[1-9]\d{0,8}$/.test(build ?? ""))
  throw new Error("Set an increasing INSTALLER_BUILD_NUMBER.");
const internalVersion = `${version}-preview.${build}`;
const config = {
  packageId: "PodeTelarMac",
  title: "PodeTelar",
  updateFeed: "https://github.com/Felipebmaitan/podetelar",
};
const feed = installerUpdateFeed(config, process.env.PODETELAR_UPDATE_FEED);
const destination = join(
  root,
  ".local/macos/releases",
  `${internalVersion}${qualification ? `-local-${Date.now()}` : ""}`,
);
if (existsSync(destination)) throw new Error("Release output already exists.");
const app = join(destination, "stage/PodeTelar.app"),
  contents = join(app, "Contents"),
  executables = join(contents, "MacOS"),
  frameworks = join(contents, "Frameworks");
mkdirSync(executables, { recursive: true });
mkdirSync(frameworks);
const mediaApp = join(contents, "Helpers/PodeTelar.app");
const mediaExecutable = join(mediaApp, "Contents/MacOS/PodeTelar");
mkdirSync(dirname(mediaExecutable), { recursive: true });
mkdirSync(join(mediaApp, "Contents/Resources"));
writeFileSync(
  join(mediaApp, "Contents/Info.plist"),
  `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleName</key><string>PodeTelar</string><key>CFBundleIdentifier</key><string>app.podetelar.media</string><key>CFBundleExecutable</key><string>PodeTelar</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>${version}</string><key>CFBundleVersion</key><string>${build}</string><key>LSMinimumSystemVersion</key><string>14.0</string><key>CFBundleIconFile</key><string>PodeTelar.icns</string><key>NSHighResolutionCapable</key><true/><key>NSLocalNetworkUsageDescription</key><string>PodeTelar connects to your room's media server.</string><key>NSScreenCaptureUsageDescription</key><string>PodeTelar shares the display or window you choose with your room.</string><key>NSAudioCaptureUsageDescription</key><string>PodeTelar shares audio from the applications you choose.</string></dict></plist>`,
);
run("cargo", ["build", "--locked", "--release", "-p", "podetelar-publisher"]);
run(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "--manifest-path",
    "apps/installed-launcher/Cargo.toml",
  ],
  {
    env: {
      ...env,
      PODETELAR_UPDATE_FEED: feed,
      PODETELAR_MACOS_STATE_TEST: "",
    },
  },
);
for (const [source, name] of [
  ["target/release/podetelar-publisher", "PodeTelar"],
  [
    "apps/installed-launcher/target/release/podetelar-installed",
    "PodeTelarInstalled",
  ],
])
  copyFileSync(
    join(root, source),
    name === "PodeTelar" ? mediaExecutable : join(executables, name),
  );
// Compile from this commit; a prior native:setup may have built an older helper.
run("xcrun", [
  "swiftc",
  "-swift-version",
  "5",
  "-warnings-as-errors",
  "-O",
  "-target",
  "arm64-apple-macos14.0",
  "apps/macos/ScreenCapture.swift",
  "-o",
  join(executables, "PodeTelar"),
]);
let plist = readFileSync(join(root, "deploy/macos/Info.plist"), "utf8");
plist = plist
  .replace(
    /(<key>CFBundleShortVersionString<\/key><string>)[^<]+/,
    `$1${version}`,
  )
  .replace(
    /(<key>CFBundleVersion<\/key><string>)[^<]+/,
    `$1${version}.${build}`,
  );
writeFileSync(join(contents, "Info.plist"), plist);
const dependencies = (path) =>
  output("otool", ["-arch", "arm64", "-L", path])
    .split("\n")
    .slice(1)
    .map((line) => line.trim().split(" (")[0]);
const system = (path) =>
  path.startsWith("/usr/lib/") || path.startsWith("/System/Library/");
const copied = new Map(),
  originals = {};
function copyNative(source, target) {
  if (copied.has(target)) return;
  const archs = output("lipo", ["-archs", source]).split(/\s+/);
  if (!archs.includes("arm64"))
    throw new Error(`Missing ARM64 slice: ${basename(source)}`);
  originals[relative(contents, target)] = {
    sha256: sha256(source),
    source: relative(runtime, source),
  };
  mkdirSync(dirname(target), { recursive: true });
  if (archs.length > 1)
    run("lipo", [source, "-thin", "arm64", "-output", target]);
  else if (source !== target) copyFileSync(source, target);
  copied.set(target, true);
  // A release must honor the advertised macOS 14 minimum, including its libraries.
  const load = output("otool", ["-l", target]);
  const minimums = [
    ...load.matchAll(
      /(?:cmd LC_BUILD_VERSION[\s\S]*?minos |cmd LC_VERSION_MIN_MACOSX[\s\S]*?version )(\d+)\.(\d+)/g,
    ),
  ].map((match) => Number(match[1]) + Number(match[2]) / 100);
  if (minimums.some((minimum) => minimum > 14))
    throw new Error(`Dependency exceeds macOS 14 minimum: ${basename(source)}`);
  for (const dependency of dependencies(source)) {
    if (system(dependency)) continue;
    const name = basename(dependency);
    if (name === basename(source) && source.endsWith(".dylib")) continue;
    if (!pin.libraries.includes(name))
      throw new Error(`Unqualified macOS library: ${name}`);
    const bundled = join(frameworks, name);
    copyNative(join(runtime, "lib", name), bundled);
    const relocated = `@loader_path/${relative(dirname(target), bundled)}`;
    run("install_name_tool", ["-change", dependency, relocated, target]);
  }
  if (target.endsWith(".dylib"))
    run("install_name_tool", [
      "-id",
      `@loader_path/${basename(target)}`,
      target,
    ]);
  // Remove build-machine search paths as well as its dependency references.
  for (const match of load.matchAll(
    /cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (.+?) \(offset/g,
  ))
    run("install_name_tool", ["-delete_rpath", match[1], target]);
}
copyNative(mediaExecutable, mediaExecutable);
for (const name of ["PodeTelarInstalled", "PodeTelar"])
  copyNative(join(executables, name), join(executables, name));
copyNative(
  join(runtime, "libexec/gstreamer-1.0/gst-plugin-scanner"),
  join(executables, "gst-plugin-scanner"),
);
for (const name of pin.plugins)
  copyNative(
    join(runtime, `lib/gstreamer-1.0/libgst${name}.dylib`),
    join(contents, `PlugIns/libgst${name}.dylib`),
  );
const icons = join(destination, "PodeTelar.iconset");
mkdirSync(icons);
for (const size of [16, 32, 128, 256, 512])
  for (const scale of [1, 2])
    run(
      "sips",
      [
        "-z",
        String(size * scale),
        String(size * scale),
        "assets/PodeTelar.png",
        "--out",
        join(icons, `icon_${size}x${size}${scale === 2 ? "@2x" : ""}.png`),
      ],
      { stdio: "pipe" },
    );
mkdirSync(join(contents, "Resources"), { recursive: true });
run("iconutil", [
  "-c",
  "icns",
  icons,
  "-o",
  join(contents, "Resources/PodeTelar.icns"),
]);
copyFileSync(
  join(contents, "Resources/PodeTelar.icns"),
  join(mediaApp, "Contents/Resources/PodeTelar.icns"),
);
const notices = join(contents, "Resources/notices");
await macosNotices(notices, env);
// Source and patch provenance ships offline, alongside third-party source archives.
run("git", [
  "archive",
  "--format=tar.gz",
  `--output=${join(notices, "PodeTelar-source.tar.gz")}`,
  "HEAD",
]);
writeFileSync(
  join(notices, "runtime-inputs.json"),
  JSON.stringify({ pin, originals }, null, 2) + "\n",
);
const identity = process.env.MACOS_SIGN_IDENTITY || "-";
for (const path of [...copied.keys()].filter(
  (path) => path !== join(executables, "PodeTelar"),
))
  run("codesign", [
    "--force",
    "--sign",
    identity,
    ...(identity === "-" ? [] : ["--options", "runtime", "--timestamp"]),
    path,
  ]);
run("codesign", [
  "--force",
  "--sign",
  identity,
  ...(identity === "-" ? [] : ["--options", "runtime", "--timestamp"]),
  "--entitlements",
  "deploy/macos/app.entitlements",
  mediaApp,
]);
run("codesign", [
  "--force",
  "--sign",
  identity,
  ...(identity === "-" ? [] : ["--options", "runtime", "--timestamp"]),
  "--entitlements",
  "deploy/macos/app.entitlements",
  app,
]);
const clean = {
  ...process.env,
  DYLD_LIBRARY_PATH: "",
  DYLD_FALLBACK_LIBRARY_PATH: "",
  GST_PLUGIN_SYSTEM_PATH_1_0: join(contents, "PlugIns"),
  GST_PLUGIN_PATH_1_0: "",
  GST_PLUGIN_SCANNER_1_0: join(executables, "gst-plugin-scanner"),
  GST_REGISTRY_1_0: join(destination, "qualification-registry.bin"),
};
// Fixture encoding exercises actual hardware; hosted CI may lack a usable GPU.
run(mediaExecutable, ["--probe"], { env: clean });
run(process.execPath, ["scripts/qualify-macos-codecs.mjs", app], {
  env: clean,
});
run("dotnet", ["tool", "restore"]);
const packages = join(destination, "packages");
run("dotnet", [
  "tool",
  "run",
  "vpk",
  "--",
  "pack",
  "--runtime",
  "osx-arm64",
  "--packId",
  config.packageId,
  "--packTitle",
  config.title,
  "--packVersion",
  internalVersion,
  "--packDir",
  app,
  "--mainExe",
  "PodeTelarInstalled",
  "--channel",
  "osx-arm64",
  "--delta",
  "None",
  "--noInst",
  "--releaseNotes",
  `docs/releases/v${version}.md`,
  "--outputDir",
  packages,
  "--signAppIdentity",
  identity,
  "--signEntitlements",
  "deploy/macos/app.entitlements",
]);
const publicDir = join(destination, "public");
mkdirSync(publicDir);
const assets = [];
const add = (path, name = basename(path)) => {
  copyFileSync(path, join(publicDir, name));
  assets.push({
    filename: name,
    size: readFileSync(path).length,
    sha256: sha256(path),
  });
};
const full = macosRelease(packages, config.packageId, internalVersion);
add(join(packages, full.FileName));
add(join(packages, "releases.osx-arm64.json"));
// vpk's portable zip contains the final update helper/manifest and signed app.
// Build a DMG from that same packaged app, so installer and updates are identical.
const unpack = join(destination, "dmg");
mkdirSync(unpack);
const portable = readdirSync(packages).find((name) =>
  name.endsWith("-Portable.zip"),
);
if (!portable) throw new Error("Missing packaged macOS app.");
run("ditto", ["-xk", join(packages, portable), unpack]);
if (!existsSync(join(unpack, "PodeTelar.app/Contents/MacOS/UpdateMac")))
  throw new Error("macOS update helper missing.");
symlinkSync("/Applications", join(unpack, "Applications"));
const dmg = join(destination, `PodeTelar-v${version}-macOS-arm64.dmg`);
run("hdiutil", [
  "create",
  "-quiet",
  "-volname",
  "PodeTelar",
  "-srcfolder",
  unpack,
  "-format",
  "UDZO",
  dmg,
]);
if (process.env.MACOS_NOTARY_PROFILE) {
  run("xcrun", [
    "notarytool",
    "submit",
    dmg,
    "--keychain-profile",
    process.env.MACOS_NOTARY_PROFILE,
    "--wait",
  ]);
  run("xcrun", ["stapler", "staple", dmg]);
}
add(dmg);
const manifest = {
  name: config.title,
  version,
  internalVersion,
  commit,
  qualification,
  platform: "macos-arm64",
  minimumMacos: "14.0",
  packageId: config.packageId,
  updateFeed: feed,
  updateChannel: "osx-arm64",
  signed: identity !== "-",
  notarized: Boolean(process.env.MACOS_NOTARY_PROFILE),
  assets,
};
writeFileSync(
  join(publicDir, "macos-manifest.json"),
  JSON.stringify(manifest, null, 2) + "\n",
);
writeFileSync(
  join(publicDir, "SHA256SUMS-macos"),
  [
    ...assets,
    {
      filename: "macos-manifest.json",
      sha256: sha256(join(publicDir, "macos-manifest.json")),
    },
  ]
    .map((asset) => `${asset.sha256}  ${asset.filename}\n`)
    .join(""),
);
writeFileSync(
  join(root, ".local/macos/latest-release.json"),
  JSON.stringify(
    { destination, publicDir, app: join(unpack, "PodeTelar.app") },
    null,
    2,
  ) + "\n",
);
console.log(
  `macOS ${qualification ? "local qualification" : "release"} assets: ${publicDir}`,
);
