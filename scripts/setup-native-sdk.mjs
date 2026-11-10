import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  statSync,
  realpathSync,
  renameSync,
  rmdirSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repository = fileURLToPath(new URL("../", import.meta.url));
const pin = JSON.parse(
  readFileSync(join(repository, "deploy/native-sdk.json"), "utf8"),
);
const base = join(repository, ".local/native-sdk");
const destination = join(base, "livekit");
const hash = (data) => createHash("sha256").update(data).digest("hex");

// Complete pinned compatibility/security patch. Never trust SFU encryption
// negotiation to disable a client-requested frame cryptor.
const marker = "            LocalTrack::Audio(_audio_track) => {\n";
const addition = `                if let libwebrtc::audio_source::RtcAudioSource::Native(source) = _audio_track.rtc_source() {
                    if source.num_channels() == 2 {
                        req.audio_features.push(proto::AudioTrackFeature::TfStereo as i32);
                    }
                }
`;
const encryptionMarker =
  "        let track_info = self.inner.rtc_engine.add_track(req).await?;\n";
const encryptionCheck = `        if EncryptionType::from(track_info.encryption()) != self.local.encryption_type {
            track.disable();
            return Err(RoomError::Internal("SFU changed requested track encryption".into()));
        }
`;
const restartingMarker =
  "    fn handle_restarting(self: &Arc<Self>, tx: oneshot::Sender<()>) {\n";
const restartingCheck = `        // Full reconnect creates new RTP cryptors. Retained keys must never be
        // reused with reset per-cryptor IV counters; require a fresh client Room.
        if self.e2ee_manager.enabled() {
            for publication in self.local_participant.track_publications().values() {
                if let Some(track) = publication.track() { track.disable(); }
            }
            let session = self.clone();
            tokio::spawn(async move { let _ = session.close(DisconnectReason::ClientInitiated).await; });
            let _ = tx.send(());
            return;
        }
`;
const restartedMarker =
  "    fn handle_restarted(self: &Arc<Self>, tx: oneshot::Sender<()>) {\n";
const restartedCheck = `        if self.e2ee_manager.enabled() {
            let _ = tx.send(());
            return;
        }
`;
const receiverMarker =
  "        if !self.initialized() || publication.encryption_type() == EncryptionType::None {\n";
const receiverCheck = `        // A protected client requires a receiver cryptor even when signaling
        // labels a track plaintext. Later metadata updates must never turn an
        // already-attached plaintext decoder into an apparently protected track.
        if !self.initialized() {
`;

function treeHash(root) {
  const digest = createHash("sha256");
  function visit(directory) {
    const entries = readdirSync(directory, { withFileTypes: true }).sort(
      (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0),
    );
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile())
        digest.update(
          `${relative(root, path).split(sep).join("/")}\0${hash(readFileSync(path))}\n`,
        );
      else
        throw new Error("Unexpected link or special file in the native SDK.");
    }
  }
  visit(root);
  return digest.digest("hex");
}

if (existsSync(destination)) {
  if (treeHash(destination) !== pin.treeSha256)
    throw new Error(
      "The generated native SDK differs from its pinned patched source. Inspect it before replacing it.",
    );
  console.log(
    "Kept the verified native SDK with stereo and strict encryption fixes.",
  );
} else {
  const downloads = join(repository, ".local/downloads");
  mkdirSync(downloads, { recursive: true });
  const archive = join(downloads, `livekit-${pin.version}.crate`);
  if (!existsSync(archive)) {
    const response = await fetch(pin.url, {
      signal: AbortSignal.timeout(60_000),
    });
    if (!response.ok) throw new Error("Native SDK download failed.");
    if (!response.body) throw new Error("The native SDK download was empty.");
    const chunks = [];
    let size = 0;
    for await (const chunk of response.body) {
      size += chunk.length;
      if (size > 4 * 1024 * 1024)
        throw new Error("Native SDK archive exceeded its size limit.");
      chunks.push(chunk);
    }
    const content = Buffer.concat(chunks);
    if (hash(content) !== pin.sha256)
      throw new Error("Native SDK archive checksum did not match the pin.");
    writeFileSync(archive, content, { flag: "wx" });
  }
  if (
    statSync(archive).size > 4 * 1024 * 1024 ||
    hash(readFileSync(archive)) !== pin.sha256
  )
    throw new Error("Native SDK archive checksum did not match the pin.");
  const listing = spawnSync("tar", ["-tzf", archive], {
    encoding: "utf8",
    windowsHide: true,
    timeout: 30_000,
  });
  if (listing.error || listing.status !== 0)
    throw new Error("Could not inspect the native SDK archive.");
  const prefix = `livekit-${pin.version}/`;
  if (
    listing.stdout
      .split(/\r?\n/)
      .filter(Boolean)
      .some(
        (path) =>
          !path.startsWith(prefix) ||
          path.split("/").includes("..") ||
          path.includes("\\"),
      )
  )
    throw new Error("Unexpected native SDK archive paths.");
  mkdirSync(base, { recursive: true });
  const stage = mkdtempSync(join(base, "stage-"));
  const extraction = spawnSync("tar", ["-xzf", archive, "-C", stage], {
    encoding: "utf8",
    windowsHide: true,
    timeout: 30_000,
  });
  if (extraction.error || extraction.status !== 0)
    throw new Error("Could not unpack the native SDK.");
  const source = join(stage, `livekit-${pin.version}`);
  const target = join(source, "src/room/participant/local_participant.rs");
  const original = readFileSync(target, "utf8").replaceAll("\r\n", "\n");
  if (
    hash(original) !== pin.originalSourceSha256 ||
    original.split(marker).length !== 2
  )
    throw new Error(
      "The native SDK patch no longer matches its pinned source.",
    );
  if (original.split(encryptionMarker).length !== 2)
    throw new Error("Native SDK encryption patch marker mismatch.");
  const patched = original
    .replace(marker, marker + addition)
    .replace(encryptionMarker, encryptionMarker + encryptionCheck);
  if (hash(patched) !== pin.patchedSourceSha256)
    throw new Error("The native SDK patch checksum did not match the pin.");
  writeFileSync(target, patched);
  const roomTarget = join(source, "src/room/mod.rs");
  const roomOriginal = readFileSync(roomTarget, "utf8").replaceAll(
    "\r\n",
    "\n",
  );
  if (
    hash(roomOriginal) !== pin.roomOriginalSourceSha256 ||
    roomOriginal.split(restartingMarker).length !== 2 ||
    roomOriginal.split(restartedMarker).length !== 2
  )
    throw new Error("Native SDK protected reconnect patch marker mismatch.");
  const roomPatched = roomOriginal
    .replace(restartingMarker, restartingMarker + restartingCheck)
    .replace(restartedMarker, restartedMarker + restartedCheck);
  if (hash(roomPatched) !== pin.roomPatchedSourceSha256)
    throw new Error("Native SDK protected reconnect patch checksum mismatch.");
  writeFileSync(roomTarget, roomPatched);
  const managerTarget = join(source, "src/room/e2ee/manager.rs");
  const managerOriginal = readFileSync(managerTarget, "utf8").replaceAll(
    "\r\n",
    "\n",
  );
  if (
    hash(managerOriginal) !== pin.managerOriginalSourceSha256 ||
    managerOriginal.split(receiverMarker).length !== 3
  )
    throw new Error(
      "Native SDK mandatory receiver cryptor patch marker mismatch.",
    );
  // Replace the first occurrence (remote receiver), retaining the local check.
  const managerPatched = managerOriginal.replace(receiverMarker, receiverCheck);
  if (hash(managerPatched) !== pin.managerPatchedSourceSha256)
    throw new Error(
      "Native SDK mandatory receiver cryptor patch checksum mismatch.",
    );
  writeFileSync(managerTarget, managerPatched);
  const actualTree = treeHash(source);
  if (actualTree !== pin.treeSha256)
    throw new Error(
      `Patched native SDK tree checksum did not match: ${actualTree}`,
    );
  // Verify both resolved paths before moving the complete directory. Never
  // replace an existing checkout or a path outside this workspace-owned base.
  const resolvedBase = realpathSync(base);
  const resolvedSource = realpathSync(source);
  if (
    !resolvedSource.startsWith(resolvedBase + sep) ||
    realpathSync(dirname(resolve(destination))) !== resolvedBase ||
    existsSync(destination)
  )
    throw new Error("Unexpected native SDK destination.");
  renameSync(resolvedSource, destination);
  rmdirSync(stage); // Empty after moving the one pinned package; no recursive deletion.
  console.log(
    "Prepared the verified native SDK with stereo and strict encryption fixes.",
  );
}
