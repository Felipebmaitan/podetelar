// MIT. The local observer may change only Glow's registry-to-path origin.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import assert from "node:assert/strict";
const label = process.argv[2];
assert.match(label ?? "", /^[a-z0-9-]{1,20}$/);
const dir = resolve(
  import.meta.dirname,
  `../.local/experiments/deep-pass/gl-${label}`,
);
const normalize = (path) =>
  readFileSync(resolve(dir, path), "utf8")
    .replaceAll("\r\n", "\n")
    .split("\n[[package]]\n")
    .map((part) => {
      if (part.startsWith('name = "egui_glow"\nversion = "0.37.0"\n'))
        return part
          .replace(/^source = .*\n/gm, "")
          .replace(/^checksum = .*\n/gm, "");
      return part;
    })
    .join("\n[[package]]\n");
assert.equal(
  normalize("Cargo.observer.lock"),
  normalize("Cargo.original.lock"),
  "Observer changed another package or dependency",
);
console.log(JSON.stringify({ observerLockVerified: true, label }));
