// MIT. Freeze post-run code only; never copies configs, databases or credentials.
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createHash } from "node:crypto";
const root = resolve(import.meta.dirname, ".."),
  out = process.argv[2]
    ? resolve(process.argv[2])
    : resolve(root, ".local/experiments/deep-pass/server-code");
if (existsSync(out)) throw Error("Server code archive is immutable");
const files = [];
function visit(folder, extension) {
  for (const entry of readdirSync(resolve(root, folder), {
    withFileTypes: true,
  })) {
    const file = join(folder, entry.name).replaceAll("\\", "/");
    if (entry.isDirectory()) visit(file, extension);
    else if (entry.isFile() && file.endsWith(extension)) files.push(file);
  }
}
for (const folder of ["apps/server/src", "packages/contracts/src"])
  visit(folder, ".ts");
for (const folder of ["apps/server/dist", "packages/contracts/dist"])
  visit(folder, ".js");
files.push("package-lock.json");
const entries = [];
for (const file of files.sort()) {
  const bytes = readFileSync(resolve(root, file)),
    target = join(out, "source", file);
  mkdirSync(dirname(target), { recursive: true });
  copyFileSync(resolve(root, file), target);
  entries.push({
    file,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    bytes: bytes.length,
  });
}
const receipt = {
  schema: 1,
  node: process.versions.node,
  files: entries,
  limits:
    "Post-run archive. Server production source has not changed since the checkpoint; validate compiled identities against the final build. No assertion that these hashes were recorded before the measurements.",
};
writeFileSync(
  join(out, "source.json"),
  JSON.stringify(receipt, null, 2) + "\n",
);
console.log(JSON.stringify({ archive: out, files: entries.length }));
