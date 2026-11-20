// MIT. Observe real tagged video texture API uploads in a private pinned Glow copy.
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { homedir } from "node:os";
import { resolve, join } from "node:path";
const root = resolve(import.meta.dirname, ".."),
  label = process.argv[2];
if (!/^[a-z0-9-]{1,20}$/.test(label ?? ""))
  throw Error("Choose a new observer label");
const out = resolve(root, `.local/experiments/deep-pass/gl-${label}`);
if (existsSync(out)) throw Error("Observer source snapshots are immutable");
const registry = resolve(
  process.env.CARGO_HOME ?? join(homedir(), ".cargo"),
  "registry/src",
);
const candidates = readdirSync(registry)
  .map((d) => join(registry, d, "egui_glow-0.37.0"))
  .filter(existsSync);
if (candidates.length !== 1)
  throw Error("Expected one installed pinned Glow source");
const source = candidates[0];
const hash = (b) => createHash("sha256").update(b).digest("hex");
const lock = readFileSync(join(root, "Cargo.lock"), "utf8");
const pin = lock.match(
  /name = "egui_glow"\r?\nversion = "0\.37\.0"\r?\nsource = "[^"\n]+"\r?\nchecksum = "([0-9a-f]+)"/,
);
const archive = resolve(
  source,
  "../../..",
  "cache",
  source.split(/[\\/]/).at(-2),
  "egui_glow-0.37.0.crate",
);
if (!pin || hash(readFileSync(archive)) !== pin[1])
  throw Error("Glow archive differs from Cargo.lock");
const entries = execFileSync("tar", ["-tf", archive], {
  encoding: "utf8",
  windowsHide: true,
})
  .trim()
  .split(/\r?\n/);
if (
  entries.some(
    (p) =>
      !p.startsWith("egui_glow-0.37.0/") || p.split(/[\\/]/).includes(".."),
  )
)
  throw Error("Unexpected pinned archive path");
mkdirSync(out, { recursive: true });
execFileSync("tar", ["-xf", archive, "-C", out], { windowsHide: true });
const copied = join(out, "egui_glow-0.37.0"),
  replace = (s, a, b) => {
    if (s.split(a).length !== 2) throw Error("Pinned Glow marker changed");
    return s.replace(a, b);
  };
for (const path of entries.filter((p) =>
  /\.(rs|glsl)$|\/Cargo.toml$/.test(p),
)) {
  const relative = path.slice("egui_glow-0.37.0/".length);
  if (
    hash(readFileSync(join(source, relative))) !==
    hash(readFileSync(join(copied, relative)))
  )
    throw Error("Installed compiled Glow sources differ from pinned archive");
}
const originals = Object.fromEntries(
  ["src/painter.rs", "src/lib.rs"].map((f) => [
    f,
    hash(readFileSync(join(copied, f))),
  ]),
);
let painter = readFileSync(join(copied, "src/painter.rs"), "utf8");
painter = replace(
  painter,
  "                self.upload_texture_srgb(delta.pos, image.size, delta.options, data);",
  "                self.upload_texture_srgb(delta.pos, image.size, delta.options, data);\n                crate::deep_gl_observer::uploaded(tex_id, data.len(), delta.pos.is_some());",
);
painter = replace(
  painter,
  "    pub fn free_texture(&mut self, tex_id: egui::TextureId) {",
  "    pub fn free_texture(&mut self, tex_id: egui::TextureId) {\n        crate::deep_gl_observer::forget(tex_id);",
);
writeFileSync(join(copied, "src/painter.rs"), painter);
const lib =
  readFileSync(join(copied, "src/lib.rs"), "utf8") +
  "\n#[doc(hidden)]\npub mod deep_gl_observer;\n";
writeFileSync(join(copied, "src/lib.rs"), lib);
const observer = readFileSync(
  join(root, "scripts/experiments/deep-gl-observer.rs"),
  "utf8",
);
writeFileSync(join(copied, "src/deep_gl_observer.rs"), observer);
writeFileSync(
  join(out, "source.json"),
  JSON.stringify(
    {
      schema: 1,
      pinnedPackageSha256: pin[1],
      originals,
      generated: Object.fromEntries(
        ["src/painter.rs", "src/lib.rs", "src/deep_gl_observer.rs"].map((f) => [
          f,
          hash(readFileSync(join(copied, f))),
        ]),
      ),
      limits:
        "Local observer only; original registry source unchanged. Counts tagged video texImage/texSubImage calls and their input payload, not physical PCIe traffic or display output.",
    },
    null,
    2,
  ),
);
console.log(JSON.stringify({ prepared: copied }));
