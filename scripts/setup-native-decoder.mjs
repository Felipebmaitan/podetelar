import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { join, relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const base = join(root, ".local/native-sdk");
const pin = JSON.parse(readFileSync(join(root, "deploy/native-decoder.json")));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const frameCryptorChanges = [
  [
    "  sender->SetEncoderToPacketizerFrameTransformer(e2ee_transformer_);\n  e2ee_transformer_->SetEnabled(false);\n",
    "  e2ee_transformer_->SetEnabled(true);\n  sender->SetEncoderToPacketizerFrameTransformer(e2ee_transformer_);\n",
  ],
  [
    "  receiver->SetDepacketizerToDecoderFrameTransformer(e2ee_transformer_);\n  e2ee_transformer_->SetEnabled(false);\n",
    "  e2ee_transformer_->SetEnabled(true);\n  receiver->SetDepacketizerToDecoderFrameTransformer(e2ee_transformer_);\n",
  ],
  [
    "  e2ee_transformer_->SetEnabled(enabled);\n",
    "  // Once attached, native protected media must never become plaintext,\n  // including during cleanup or reattachment to an already pumped receiver.\n  static_cast<void>(enabled);\n  e2ee_transformer_->SetEnabled(true);\n",
  ],
];
const treeHash = (directory) => {
  const digest = createHash("sha256");
  const visit = (path) => {
    for (const entry of readdirSync(path, { withFileTypes: true }).sort(
      (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0),
    )) {
      const file = join(path, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile())
        digest.update(
          `${relative(directory, file).replaceAll("\\", "/")}\0${hash(readFileSync(file))}\n`,
        );
      else throw new Error("Unexpected link in decoder source.");
    }
  };
  visit(directory);
  return digest.digest("hex");
};
const destination = join(base, "webrtc-sys");
const patch = readFileSync(
  join(root, "apps/publisher/decoder/freedomstream_decoder.h"),
  "utf8",
).replaceAll("\r\n", "\n");
if (hash(patch) !== pin.adapterSha256)
  throw new Error("Decoder adapter differs from its pin.");
if (existsSync(destination)) {
  if (treeHash(destination) !== pin.treeSha256)
    throw new Error("Generated decoder SDK differs from its pin.");
  console.log("Kept verified native Direct3D11 decoder SDK.");
} else {
  const downloads = join(root, ".local/downloads");
  mkdirSync(downloads, { recursive: true });
  const archive = join(downloads, `webrtc-sys-${pin.version}.crate`);
  if (!existsSync(archive)) {
    const response = await fetch(pin.url, {
      signal: AbortSignal.timeout(60_000),
    });
    if (!response.ok || !response.body)
      throw new Error("Decoder SDK download failed.");
    let size = 0;
    const chunks = [];
    for await (const chunk of response.body) {
      size += chunk.length;
      if (size > 16 * 1024 * 1024)
        throw new Error("Decoder SDK archive too large.");
      chunks.push(chunk);
    }
    const bytes = Buffer.concat(chunks);
    if (hash(bytes) !== pin.sha256)
      throw new Error("Decoder SDK archive checksum mismatch.");
    writeFileSync(archive, bytes, { flag: "wx" });
  }
  if (hash(readFileSync(archive)) !== pin.sha256)
    throw new Error("Decoder SDK archive checksum mismatch.");
  const prefix = `webrtc-sys-${pin.version}/`;
  const entries = execFileSync("tar", ["-tzf", archive], {
    encoding: "utf8",
    windowsHide: true,
  })
    .split(/\r?\n/)
    .filter(Boolean);
  if (
    entries.some(
      (path) =>
        !path.startsWith(prefix) ||
        path.includes("\\") ||
        path.split("/").includes(".."),
    )
  )
    throw new Error("Unexpected decoder archive paths.");
  mkdirSync(base, { recursive: true });
  const stage = mkdtempSync(join(base, "decoder-stage-"));
  execFileSync("tar", ["-xzf", archive, "-C", stage], { windowsHide: true });
  const source = join(stage, `webrtc-sys-${pin.version}`);
  const modify = (path, marker, replacement) => {
    const file = join(source, path);
    const original = readFileSync(file, "utf8").replaceAll("\r\n", "\n");
    if (original.split(marker).length !== 2)
      throw new Error(`Decoder patch marker changed in ${path}.`);
    writeFileSync(file, original.replace(marker, replacement));
  };
  modify(
    "src/video_decoder_factory.cpp",
    "namespace livekit_ffi {",
    '#ifdef _WIN32\n#include "freedomstream_decoder.h"\n#endif\n\nnamespace livekit_ffi {',
  );
  modify(
    "src/video_decoder_factory.cpp",
    "#ifdef __APPLE__",
    "#ifdef _WIN32\n  factories_.push_back(std::make_unique<freedomstream::DecoderFactory>());\n#endif\n#ifdef __APPLE__",
  );
  modify(
    "build.rs",
    '        "windows" => {',
    `        "windows" => {
            let gst = PathBuf::from(env::var("GSTREAMER_1_0_ROOT_MSVC_X86_64").expect("Bundled GStreamer development runtime is required"));
            builder.include(gst.join("include/gstreamer-1.0"))
                .include(gst.join("include/glib-2.0"))
                .include(gst.join("lib/glib-2.0/include"));
            println!("cargo:rustc-link-search=native={}", gst.join("lib").display());
            for lib in ["gstreamer-1.0", "gstapp-1.0", "gstvideo-1.0", "gobject-2.0", "glib-2.0"] {
                println!("cargo:rustc-link-lib=dylib={lib}");
            }
            println!("cargo:rerun-if-env-changed=GSTREAMER_1_0_ROOT_MSVC_X86_64");
            println!("cargo:rerun-if-changed=src/freedomstream_decoder.h");`,
  );
  const frameCryptor = join(source, "src/frame_cryptor.cpp");
  if (
    hash(readFileSync(frameCryptor, "utf8").replaceAll("\r\n", "\n")) !==
    pin.frameCryptorOriginalSha256
  )
    throw new Error("Frame cryptor original source mismatch.");
  for (const [marker, replacement] of frameCryptorChanges)
    modify("src/frame_cryptor.cpp", marker, replacement);
  if (
    hash(readFileSync(frameCryptor, "utf8").replaceAll("\r\n", "\n")) !==
    pin.frameCryptorPatchedSha256
  )
    throw new Error("Frame cryptor protected source mismatch.");
  writeFileSync(join(source, "src/freedomstream_decoder.h"), patch);
  const actual = treeHash(source);
  if (actual !== pin.treeSha256)
    throw new Error(`Decoder patched tree mismatch: ${actual}`);
  renameSync(source, destination);
  console.log("Prepared verified native Direct3D11 decoder SDK.");
}
