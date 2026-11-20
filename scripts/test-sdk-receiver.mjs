// Exercise the pinned SDK's encryption negotiation and receiver attachment checks.
import { spawnSync } from "node:child_process";
import {
  cpSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
if (
  process.platform !== "win32" ||
  process.versions.node.split(".")[0] !== "24"
)
  throw new Error("Use Windows and Node 24 for the native SDK regression.");
for (const script of ["setup-native-sdk.mjs", "setup-native-decoder.mjs"]) {
  const verify = spawnSync(process.execPath, [join(root, "scripts", script)], {
    cwd: root,
    windowsHide: true,
    stdio: "inherit",
  });
  if (verify.status !== 0) process.exit(verify.status ?? 1);
}
const local = join(root, ".local");
mkdirSync(local, { recursive: true });
const run = mkdtempSync(join(local, "sdk-receiver-test-"));
cpSync(join(local, "native-sdk/livekit"), run, { recursive: true });
copyFileSync(join(root, "Cargo.lock"), join(run, "Cargo.lock"));
const manifest = join(run, "Cargo.toml");
const original = readFileSync(manifest, "utf8");
if (original.includes("[workspace]") || original.includes("[patch.crates-io]"))
  throw new Error("Unexpected SDK test workspace configuration.");
const patchPaths = ["webrtc-sys", "libwebrtc"].map((name) => {
  const path = join(local, "native-sdk", name).replaceAll("\\", "/");
  return `${name} = { path = ${JSON.stringify(path)} }`;
});
writeFileSync(
  manifest,
  `${original}\n[workspace]\n\n[patch.crates-io]\n${patchPaths.join("\n")}\n`,
);
const gst = resolve(
  process.env.PODETELAR_GSTREAMER ?? join(local, "gstreamer"),
);
const result = spawnSync("cargo", ["test", "--lib", "podetelar_encryption_"], {
  cwd: run,
  windowsHide: true,
  stdio: "inherit",
  env: {
    ...process.env,
    CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(root, "target"),
    PATH: `${join(gst, "bin")};${process.env.PATH}`,
    PKG_CONFIG_PATH: join(gst, "lib/pkgconfig"),
    GSTREAMER_1_0_ROOT_MSVC_X86_64: gst,
  },
});
process.exitCode = result.status ?? 1;
