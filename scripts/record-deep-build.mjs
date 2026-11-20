// MIT. Freeze source identity without including ignored configs/media/credentials.
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  writeFileSync,
  copyFileSync,
  existsSync,
} from "node:fs";
import { resolve, dirname, join } from "node:path";
import { createHash } from "node:crypto";
const label = process.argv[2],
  exe = process.argv[3];
if (!/^[a-z0-9-]{1,30}$/.test(label ?? ""))
  throw Error("Choose a source label");
const root = resolve(import.meta.dirname, ".."),
  out = resolve(root, ".local/experiments/deep-pass", `source-${label}`);
if (existsSync(out))
  throw Error("Build source snapshots are immutable; choose a new label");
mkdirSync(out, { recursive: true });
const files = execFileSync(
  "git",
  [
    "ls-files",
    "-co",
    "--exclude-standard",
    "--",
    "Cargo.toml",
    "Cargo.lock",
    "apps/publisher",
    "scripts/experiments",
    "scripts/prepare-deep-decoder.mjs",
    "scripts/prepare-deep-gl.mjs",
    "scripts/check-deep-gl-lock.mjs",
    "scripts/build-deep-decoder.ps1",
    "scripts/build-deep-sdk.ps1",
    "scripts/record-deep-build.mjs",
    "scripts/benchmark-deep-decoder.ps1",
    "scripts/sample-native-process.ps1",
    "scripts/summarize-deep-performance.mjs",
    "scripts/local-performance-room.mjs",
    "scripts/run-deep-performance.mjs",
  ],
  { cwd: root, encoding: "utf8", windowsHide: true },
)
  .trim()
  .split(/\r?\n/)
  .filter(Boolean);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const entries = [];
for (const file of [...new Set(files)].sort()) {
  const data = readFileSync(resolve(root, file));
  const path = join(out, "source", file);
  mkdirSync(dirname(path), { recursive: true });
  copyFileSync(resolve(root, file), path);
  entries.push({ file, sha256: hash(data) });
}
const manifest = {
  schema: 1,
  base: "e93b71d437810192bd1648e65088f1a6b9196754",
  files: entries,
  treeSha256: hash(JSON.stringify(entries)),
  executable: exe
    ? { path: exe, sha256: hash(readFileSync(resolve(root, exe))) }
    : null,
};
writeFileSync(join(out, "manifest.json"), JSON.stringify(manifest, null, 2));
console.log(
  JSON.stringify({
    source: out,
    files: entries.length,
    treeSha256: manifest.treeSha256,
    exeSha256: manifest.executable?.sha256,
  }),
);
