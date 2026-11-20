import {
  copyFileSync,
  cpSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  existsSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import { execFileSync } from "node:child_process";
import {
  root,
  runtime,
  pin,
  downloadPinned,
  sha256,
} from "./macos-runtime.mjs";
export async function macosNotices(destination, env) {
  mkdirSync(destination, { recursive: true });
  for (const name of pin.licenseDirectories) {
    if (
      name === "gst-plugins-good-1.0" &&
      !existsSync(join(runtime, "share/licenses", name))
    )
      continue;
    cpSync(
      join(runtime, "share/licenses", name),
      join(destination, "runtime", name),
      { recursive: true },
    );
  }
  copyFileSync(
    join(runtime, "share/versions.txt"),
    join(destination, "runtime-versions.txt"),
  );
  copyFileSync(join(root, "LICENSE"), join(destination, "PodeTelar-MIT.txt"));
  copyFileSync(
    join(root, "assets/fonts/OFL-Inter.txt"),
    join(destination, "Inter-OFL.txt"),
  );
  copyFileSync(
    join(root, "deploy/windows/Velopack-LICENSE.txt"),
    join(destination, "Velopack-LICENSE.txt"),
  );
  const overrides = JSON.parse(
    readFileSync(join(root, "deploy/license-overrides.json")),
  );
  const metadata = (args) =>
    JSON.parse(
      execFileSync(
        "cargo",
        [
          "metadata",
          "--locked",
          "--format-version=1",
          "--filter-platform=aarch64-apple-darwin",
          ...args,
        ],
        { cwd: root, env, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
      ),
    );
  const seen = new Set(),
    index = [];
  for (const graph of [
    metadata([]),
    metadata(["--manifest-path", "apps/installed-launcher/Cargo.toml"]),
  ]) {
    const included = new Set(graph.resolve.nodes.map((node) => node.id));
    for (const pkg of graph.packages.filter(
      (pkg) => included.has(pkg.id) && !pkg.name.startsWith("podetelar-"),
    )) {
      const key = `${pkg.name}@${pkg.version}`;
      if (seen.has(key)) continue;
      seen.add(key);
      index.push(`${key}: ${pkg.license ?? "see included notices"}`);
      if (key === "velopack@1.2.161") continue;
      const directory = dirname(pkg.manifest_path);
      const files = readdirSync(directory, {
        recursive: true,
        withFileTypes: true,
      }).filter(
        (entry) =>
          entry.isFile() &&
          /^(?:licen[cs]e|copying|notice|ofl)(?:[._-]|$)/i.test(entry.name),
      );
      if (!files.length) {
        if (!overrides[key]?.length)
          throw new Error(`Missing license text: ${key}`);
        for (const [i, override] of overrides[key].entries()) {
          const source = join(root, override.file);
          if (sha256(source) !== override.sha256)
            throw new Error("License override checksum mismatch.");
          const target = join(
            destination,
            "cargo",
            key,
            `${i}-${basename(source)}`,
          );
          mkdirSync(dirname(target), { recursive: true });
          copyFileSync(source, target);
        }
      } else
        for (const entry of files) {
          const source = join(entry.parentPath, entry.name),
            target = join(
              destination,
              "cargo",
              key,
              relative(directory, source),
            );
          mkdirSync(dirname(target), { recursive: true });
          copyFileSync(source, target);
        }
    }
  }
  writeFileSync(
    join(destination, "cargo-dependencies.txt"),
    index.join("\n") + "\n",
  );
  const buildRoot = join(root, "target/release/build");
  const webrtc = readdirSync(buildRoot, {
    recursive: true,
    withFileTypes: true,
  }).find(
    (entry) =>
      entry.isFile() &&
      entry.name === "LICENSE.md" &&
      entry.parentPath.includes("mac-arm64-release-webrtc-"),
  );
  if (!webrtc) throw new Error("Missing native WebRTC third-party notices.");
  copyFileSync(
    join(webrtc.parentPath, webrtc.name),
    join(destination, "WebRTC-third-party-LICENSE.md"),
  );
  const sources = JSON.parse(
    readFileSync(join(root, "deploy/portable-runtime.json")),
  ).sources.filter((source) => !/directx|gstreamer-202e/.test(source.filename));
  for (const source of sources)
    if (source.filename === "libjpeg-turbo-3.1.0.tar.gz")
      source.url =
        "https://github.com/libjpeg-turbo/libjpeg-turbo/releases/download/3.1.0/libjpeg-turbo-3.1.0.tar.gz";
  sources.push(...pin.sources);
  for (const source of sources) {
    const cache = join(
      root,
      ".local/downloads/portable-sources",
      source.filename,
    );
    await downloadPinned(source.url, cache, source.sha256, 128 * 1024 * 1024);
    mkdirSync(join(destination, "sources"), { recursive: true });
    copyFileSync(cache, join(destination, "sources", source.filename));
    const entries = execFileSync("tar", ["-tf", cache], {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    })
      .split(/\r?\n/)
      .filter(
        (entry) =>
          entry.split("/").length === 2 &&
          /^(?:copying|licen[cs]e|notice)(?:[._-]|$)/i.test(basename(entry)) &&
          !entry.includes("..") &&
          !entry.startsWith("/"),
      );
    for (const entry of entries) {
      const license = execFileSync("tar", ["-xOf", cache, entry], {
        maxBuffer: 4 * 1024 * 1024,
      });
      const target = join(
        destination,
        "source-licenses",
        source.filename,
        basename(entry),
      );
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, license);
    }
  }
  writeFileSync(
    join(destination, "source-manifest.json"),
    JSON.stringify(sources, null, 2) + "\n",
  );
  writeFileSync(
    join(destination, "README.txt"),
    "PodeTelar application code is MIT licensed. Bundled libraries retain their licenses. Only the allowlisted LGPL/MIT/BSD/Apache runtime and its dependency closure are shipped. Complete LGPL library sources and Cerbero build recipes accompany the app. Relocated dynamic libraries can be replaced by compatible modified builds; re-sign the bundle ad hoc with codesign --force --deep --sign - PodeTelar.app after modification. MoltenVK retains Apache-2.0. Codec patent licensing and physical A/V synchronization are separate from software licensing and capture tests.\n",
  );
}
