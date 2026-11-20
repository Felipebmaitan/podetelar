// MIT. Expose a typed audio handle from the pinned Apache-2.0 Rust wrapper.
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
const pin = JSON.parse(readFileSync(join(root, "deploy/native-audio.json")));
const hash = (data) => createHash("sha256").update(data).digest("hex");
const tree = (directory) => {
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
      else throw Error("Unexpected audio SDK source link.");
    }
  };
  visit(directory);
  return digest.digest("hex");
};
const base = join(root, ".local/native-sdk");
const destination = join(base, "libwebrtc");
if (existsSync(destination)) {
  if (tree(destination) !== pin.treeSha256)
    throw Error("Generated audio SDK differs from its pin.");
} else {
  mkdirSync(join(root, ".local/downloads"), { recursive: true });
  const archive = join(root, `.local/downloads/libwebrtc-${pin.version}.crate`);
  if (!existsSync(archive)) {
    const response = await fetch(pin.url, {
      signal: AbortSignal.timeout(60_000),
    });
    if (!response.ok || !response.body)
      throw Error("Audio SDK download failed.");
    let size = 0;
    const chunks = [];
    for await (const chunk of response.body) {
      size += chunk.length;
      if (size > 8 * 1024 * 1024) throw Error("Audio SDK archive too large.");
      chunks.push(chunk);
    }
    const bytes = Buffer.concat(chunks);
    if (hash(bytes) !== pin.sha256)
      throw Error("Audio SDK archive checksum mismatch.");
    writeFileSync(archive, bytes, { flag: "wx" });
  }
  if (hash(readFileSync(archive)) !== pin.sha256)
    throw Error("Audio SDK archive checksum mismatch.");
  const entries = execFileSync("tar", ["-tzf", archive], {
    encoding: "utf8",
    windowsHide: true,
  })
    .split(/\r?\n/)
    .filter(Boolean);
  if (
    entries.some(
      (path) =>
        !path.startsWith(`libwebrtc-${pin.version}/`) ||
        path.includes("\\") ||
        path.split("/").includes(".."),
    )
  )
    throw Error("Unexpected audio SDK archive paths.");
  mkdirSync(base, { recursive: true });
  const stage = mkdtempSync(join(base, "audio-stage-"));
  execFileSync("tar", ["-xzf", archive, "-C", stage], { windowsHide: true });
  const source = join(stage, `libwebrtc-${pin.version}`);
  const file = join(source, "src/audio_track.rs");
  const original = readFileSync(file, "utf8").replaceAll("\r\n", "\n");
  const marker = "impl RtcAudioTrack {\n";
  if (original.split(marker).length !== 2)
    throw Error("Audio SDK handle marker changed.");
  writeFileSync(
    file,
    original.replace(
      marker,
      marker +
        `    /// PodeTelar's pinned adapter; no ownership is transferred.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_audio_handle(&self) -> cxx::SharedPtr<webrtc_sys::audio_track::ffi::AudioTrack> {
        self.handle.sys_handle.clone()
    }
`,
    ),
  );
  const transceiver = join(source, "src/rtp_transceiver.rs");
  const transceiverSource = readFileSync(transceiver, "utf8").replaceAll(
    "\r\n",
    "\n",
  );
  const transceiverMarker = "impl RtpTransceiver {\n";
  if (transceiverSource.split(transceiverMarker).length !== 2)
    throw Error("Audio generation adapter marker changed.");
  writeFileSync(
    transceiver,
    transceiverSource.replace(
      transceiverMarker,
      transceiverMarker +
        `    /// Receiver generation captured before native track event dispatch.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn playback_generation(&self) -> u64 {
        self.handle.sys_handle.playback_generation()
    }
`,
    ),
  );
  const actual = tree(source);
  if (actual !== pin.treeSha256)
    throw Error(`Audio SDK patched tree mismatch: ${actual}`);
  renameSync(source, destination);
}
