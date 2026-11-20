import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  closeSync,
  copyFileSync,
  existsSync,
  lstatSync,
  readlinkSync,
  symlinkSync,
  unlinkSync,
  mkdirSync,
  openSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  writeSync,
} from "node:fs";
import { join, resolve } from "node:path";
export const root = resolve(import.meta.dirname, "..");
export const runtime = join(root, ".local/macos/gstreamer");
export const pin = JSON.parse(
  readFileSync(join(root, "deploy/macos/runtime.json")),
);
export const sha256 = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
export function macosEnvironment() {
  return {
    ...process.env,
    CARGO_PROFILE_RELEASE_STRIP: "none",
    CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP: "none",
    MACOSX_DEPLOYMENT_TARGET: "14.0",
    PKG_CONFIG_PATH: join(runtime, "lib/pkgconfig"),
    DYLD_LIBRARY_PATH: join(runtime, "lib"),
    GST_PLUGIN_SYSTEM_PATH_1_0: join(root, ".local/macos/plugins"),
    GST_PLUGIN_PATH_1_0: "",
    GST_PLUGIN_SCANNER_1_0: join(
      runtime,
      "libexec/gstreamer-1.0/gst-plugin-scanner",
    ),
    GST_REGISTRY_1_0: join(root, ".local/macos/development-registry.bin"),
    RUSTFLAGS: `-C link-arg=-Wl,-rpath,${join(runtime, "lib")}`,
  };
}
// Range requests keep large upstream packages bounded and allow parallel transfer.
// The entire archive must match the committed upstream checksum before extraction.
export async function downloadPinned(url, path, digest, limit = 1024 ** 3) {
  if (existsSync(path) && sha256(path) === digest) return;
  if (limit <= 128 * 1024 * 1024) {
    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        const response = await fetch(url, {
          signal: AbortSignal.timeout(120_000),
        });
        if (!response.ok || !response.body)
          throw new Error(`Source HTTP ${response.status}.`);
        const chunks = [];
        let total = 0;
        for await (const chunk of response.body) {
          total += chunk.length;
          if (total > limit) throw new Error("Source archive too large.");
          chunks.push(chunk);
        }
        const bytes = Buffer.concat(chunks);
        if (createHash("sha256").update(bytes).digest("hex") !== digest)
          throw new Error("Source checksum mismatch.");
        mkdirSync(resolve(path, ".."), { recursive: true });
        writeFileSync(path, bytes);
        return;
      } catch (error) {
        if (attempt === 2)
          throw new Error(`Could not verify source ${url}`, { cause: error });
      }
    }
  }
  const head = await fetch(url, {
    method: "HEAD",
    signal: AbortSignal.timeout(30_000),
  });
  const size = Number(head.headers.get("content-length"));
  if (!head.ok || size > limit) throw new Error("Invalid native archive size.");
  if (
    !Number.isSafeInteger(size) ||
    size <= 0 ||
    head.headers.get("accept-ranges") !== "bytes"
  ) {
    const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
    if (!response.ok || !response.body)
      throw new Error("Source download failed.");
    const chunks = [];
    let total = 0;
    for await (const chunk of response.body) {
      total += chunk.length;
      if (total > limit) throw new Error("Source archive exceeds its limit.");
      chunks.push(chunk);
    }
    mkdirSync(resolve(path, ".."), { recursive: true });
    writeFileSync(path, Buffer.concat(chunks));
    if (sha256(path) !== digest)
      throw new Error("Source archive checksum mismatch.");
    return;
  }
  mkdirSync(resolve(path, ".."), { recursive: true });
  const file = openSync(path, "w");
  let next = 0;
  try {
    const chunkSize = 1024 * 1024;
    const results = await Promise.allSettled(
      Array.from({ length: 8 }, async () => {
        while (next < size) {
          const start = next;
          next += chunkSize;
          const end = Math.min(size - 1, start + chunkSize - 1);
          let bytes;
          for (let attempt = 0; attempt < 3; attempt++) {
            try {
              const response = await fetch(url, {
                headers: { Range: `bytes=${start}-${end}` },
                signal: AbortSignal.timeout(60_000),
              });
              if (
                response.status !== 206 ||
                response.headers.get("content-range") !==
                  `bytes ${start}-${end}/${size}`
              )
                throw new Error("Invalid archive range.");
              bytes = Buffer.from(await response.arrayBuffer());
              if (bytes.length !== end - start + 1)
                throw new Error("Truncated archive range.");
              break;
            } catch (error) {
              if (attempt === 2) throw error;
            }
          }
          writeSync(file, bytes, 0, bytes.length, start);
        }
      }),
    );
    for (const result of results)
      if (result.status === "rejected") throw result.reason;
  } finally {
    closeSync(file);
  }
  if (sha256(path) !== digest)
    throw new Error("Native archive checksum mismatch.");
}
export async function setupMacosRuntime() {
  mkdirSync(runtime, { recursive: true });
  for (const [kind, item] of [
    ["runtime", pin.runtime],
    ["devel", pin.development],
  ]) {
    const archive = join(root, `.local/downloads/gstreamer-macos-${kind}.pkg`);
    await downloadPinned(
      `https://gstreamer.freedesktop.org/data/pkg/osx/${pin.version}/${item.filename}`,
      archive,
      item.sha256,
    );
    const expanded = join(root, `.local/macos/${kind}-pkg`);
    if (!existsSync(expanded))
      execFileSync("pkgutil", ["--expand-full", archive, expanded]);
    for (const component of readdirSync(expanded).filter((name) =>
      name.endsWith(".pkg"),
    )) {
      const info = readFileSync(
        join(expanded, component, "PackageInfo"),
        "utf8",
      );
      const payload = join(expanded, component, "Payload");
      if (
        info.includes(
          'install-location="/Library/Frameworks/GStreamer.framework/Versions/1.0"',
        ) &&
        existsSync(payload)
      )
        merge(payload, runtime);
    }
  }
  const plugins = join(root, ".local/macos/plugins");
  mkdirSync(plugins, { recursive: true });
  for (const name of pin.plugins) {
    const path = join(plugins, `libgst${name}.dylib`);
    if (!existsSync(path))
      symlinkSync(join(runtime, `lib/gstreamer-1.0/libgst${name}.dylib`), path);
  }
  const pc = join(runtime, "lib/pkgconfig");
  for (const name of readdirSync(pc).filter((name) => name.endsWith(".pc"))) {
    const path = join(pc, name);
    writeFileSync(
      path,
      readFileSync(path, "utf8").replace(/^prefix=.*$/m, `prefix=${runtime}`),
    );
  }
}

function merge(source, destination) {
  mkdirSync(destination, { recursive: true });
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const from = join(source, entry.name),
      to = join(destination, entry.name);
    if (entry.isDirectory()) merge(from, to);
    else if (entry.isSymbolicLink()) {
      try {
        if (lstatSync(to).isSymbolicLink()) unlinkSync(to);
        else throw new Error("Unexpected SDK path conflict.");
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
      }
      symlinkSync(readlinkSync(from), to);
    } else copyFileSync(from, to);
  }
}
