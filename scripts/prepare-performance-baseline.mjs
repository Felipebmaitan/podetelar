// Export an immutable application baseline and add only the shared measurement
// harness/counters. This is a source fixture, not another git checkout or app.
import { execFileSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { resolve, join } from "node:path";
import { randomUUID, createHash } from "node:crypto";
const root = resolve(import.meta.dirname, "..");
const base = "93d6ee13d72898fa52b22305ff0f64e7ea05aea2";
const output = join(
  root,
  ".local/experiments",
  `baseline-source-${randomUUID()}`,
);
mkdirSync(output, { recursive: true });
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const execute = (file, args) =>
  execFileSync(file, args, { cwd: root, windowsHide: true, stdio: "pipe" });
const originalAdapter = execute("git", [
  "show",
  `${base}:apps/publisher/decoder/freedomstream_decoder.h`,
]);
const currentAdapter = readFileSync(
  join(root, ".local/native-sdk/webrtc-sys/src/freedomstream_decoder.h"),
);
if (hash(originalAdapter) !== hash(currentAdapter))
  throw new Error(
    "Prepare the original pinned decoder source before exporting this baseline.",
  );
const archive = join(output, "source.tar");
execute("git", [
  "archive",
  "--format=tar",
  `--output=${archive}`,
  base,
  "Cargo.toml",
  "Cargo.lock",
  "apps/publisher",
  "apps/portable",
  "assets/windows",
  "deploy/native-sdk.json",
  "deploy/native-decoder.json",
]);
execute("tar", ["-xf", archive, "-C", output]);
mkdirSync(join(output, ".local/native-sdk"), { recursive: true });
for (const name of ["livekit", "webrtc-sys"])
  cpSync(
    join(root, ".local/native-sdk", name),
    join(output, ".local/native-sdk", name),
    { recursive: true, errorOnExist: true, force: false },
  );
const changes = [];
function edit(relative, action) {
  const path = join(output, relative);
  const before = readFileSync(path, "utf8").replaceAll("\r\n", "\n");
  const after = action(before);
  if (before === after)
    throw new Error(`Baseline instrumentation was not applied: ${relative}`);
  writeFileSync(path, after);
  changes.push({
    path: relative,
    beforeSha256: hash(before),
    afterSha256: hash(after),
  });
}
function once(text, marker, replacement) {
  if (text.split(marker).length !== 2)
    throw new Error("Baseline instrumentation marker changed.");
  return text.replace(marker, replacement);
}
edit("apps/publisher/Cargo.toml", (text) =>
  once(
    text,
    "[dependencies]",
    "[features]\nperformance-experiments = []\n\n[dependencies]",
  ),
);
edit("apps/publisher/src/sources.rs", (text) => {
  text = once(
    text,
    "mod windows;",
    '#[path = "sources/windows.rs"]\nmod windows;',
  );
  return once(
    text,
    "        windows::enumerate()",
    `        let sources=windows::enumerate();
        #[cfg(feature="performance-experiments")]
        let sources={let mut sources=sources;if std::env::var("FREEDOMSTREAM_PERFORMANCE_FIXTURE").is_ok_and(|value|value=="1"){sources.push(fixture());}sources};
        sources`,
  );
});
edit("apps/publisher/src/playback.rs", (text) => {
  text = once(
    text,
    "pub type Picture =",
    `#[derive(Default)]
pub struct Presentation {pub converted:AtomicU64,pub uploaded:AtomicU64}
pub static SDK_MESSAGES:AtomicU64=AtomicU64::new(0);
pub type Picture =`,
  );
  text = once(
    text,
    "    pub picture: Picture,",
    "    pub picture: Picture,\n    pub presentation:Arc<Presentation>,",
  );
  text = once(
    text,
    "struct Watch {\n    picture: Picture,",
    "struct Watch {\n    picture: Picture,\n    presentation:Arc<Presentation>,",
  );
  text = once(
    text,
    "                picture: Arc::default(),",
    "                picture: Arc::default(),\n                presentation:Arc::default(),",
  );
  text = once(
    text,
    "                        let picture = watch.picture.clone();",
    "                        let picture = watch.picture.clone();\n                        let presentation=watch.presentation.clone();",
  );
  text = once(
    text,
    "                                frames.fetch_add(1, Ordering::Relaxed);",
    "                                frames.fetch_add(1, Ordering::Relaxed);\n                                presentation.converted.fetch_add(1,Ordering::Relaxed);",
  );
  return once(
    text,
    "                frames: watch.map(|w| w.frames.clone()).unwrap_or_default(),",
    "                presentation:watch.map(|w|w.presentation.clone()).unwrap_or_default(),\n                frames: watch.map(|w| w.frames.clone()).unwrap_or_default(),",
  );
});
edit("apps/publisher/src/ui.rs", (text) => {
  text =
    `#[cfg(feature="performance-experiments")]\n#[allow(dead_code)]\n#[path="ui/experiment.rs"]\npub mod experiment;\n` +
    text;
  text = once(
    text,
    "                        picture,\n                        frames:",
    "                        picture,\n                        presentation:std::sync::Arc::default(),\n                        frames:",
  );
  return once(
    text,
    "            && let Some(image) = picture.take()\n        {",
    "            && let Some(image) = picture.take()\n        {\n            tile.presentation.uploaded.fetch_add(1,Ordering::Relaxed);",
  );
});
const uiDirectory = join(output, "apps/publisher/src/ui");
mkdirSync(uiDirectory, { recursive: true });
const harness = readFileSync(
  join(root, "apps/publisher/src/ui/experiment.rs"),
  "utf8",
)
  .split(/\r?\n/)
  .filter(
    (line) =>
      !line.includes("microbench") && !line.includes('#[path = "micro.rs"]'),
  )
  .join("\n");
writeFileSync(join(uiDirectory, "experiment.rs"), harness);
let example = readFileSync(
  join(root, "apps/publisher/examples/room-benchmark.rs"),
  "utf8",
).replaceAll("\r\n", "\n");
const begin = example.indexOf("    if std::env::args().nth(1)");
const end = example.indexOf("    let mut input = String::new();", begin);
if (begin < 0 || end < 0)
  throw new Error("Shared example CLI structure changed.");
example = example.slice(0, begin) + example.slice(end);
writeFileSync(
  join(output, "apps/publisher/examples/room-benchmark.rs"),
  example,
);
writeFileSync(
  join(output, "manifest.json"),
  JSON.stringify(
    {
      schema: 1,
      base,
      changes,
      harnessSha256: hash(harness),
      exampleSha256: hash(example),
      decoderAdapterSha256: hash(originalAdapter),
      limits:
        "Original application logic retained. Added counters at original conversion/upload points, explicit source module path, opt-in synthetic publisher source and identical UI benchmark driver. No demand gating, pooling, setter coalescing, JPEG/profile changes or audio ownership changes.",
    },
    null,
    2,
  ),
);
console.log(
  JSON.stringify({ output, base, manifest: join(output, "manifest.json") }),
);
