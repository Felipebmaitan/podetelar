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
const destination = join(base, "livekit-0.10.1-recovery");
const hash = (data) => createHash("sha256").update(data).digest("hex");

// Remove this patch when the pinned SDK announces NativeAudioSource's stereo
// channels upstream.
const marker = "            LocalTrack::Audio(_audio_track) => {\n";
const addition = `                if let libwebrtc::audio_source::RtcAudioSource::Native(source) = _audio_track.rtc_source() {
                    if source.num_channels() == 2 {
                        req.audio_features.push(proto::AudioTrackFeature::TfStereo as i32);
                    }
                }
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

function patchMediaRecovery(source) {
  const patch = (file, replacements) => {
    const path = join(source, "src/rtc_engine", file);
    let code = readFileSync(path, "utf8").replaceAll("\r\n", "\n");
    for (const [marker, replacement] of replacements) {
      if (code.split(marker).length !== 2)
        throw new Error(`Native media recovery marker changed in ${file}.`);
      code = code.replace(marker, replacement);
    }
    writeFileSync(path, code);
  };
  patch("rtc_session.rs", [
    [
      "    Close {\n        source: String,",
      `    MediaDisconnected { target: SignalTarget, generation: u32 },
    Close {
        source: String,`,
    ],
    [
      "    pub fn has_published(&self) -> bool {",
      `    pub fn media_still_disconnected(&self, target: SignalTarget, generation: u32) -> bool {
        !self.inner.closed.load(Ordering::Acquire)
            && !self.inner.disconnecting.load(Ordering::Acquire)
            && self.inner.media_transport(target).is_some_and(|transport| {
                transport.disconnect_generation() == generation
                    && transport.peer_connection().connection_state()
                        == PeerConnectionState::Disconnected
            })
    }

    pub fn has_published(&self) -> bool {`,
    ],
    [
      "    async fn on_rtc_event(&self, event: RtcEvent) -> EngineResult<()> {",
      `    fn media_transport(&self, target: SignalTarget) -> Option<&PeerTransport> {
        match target {
            SignalTarget::Publisher
                if self.single_pc_mode || self.has_published.load(Ordering::Acquire) =>
            {
                Some(&self.publisher_pc)
            }
            SignalTarget::Subscriber if self.subscriber_primary => self.subscriber_pc.as_ref(),
            _ => None,
        }
    }

    // Give libwebrtc time to switch candidate pairs on its own. A VPN or
    // adapter change can otherwise leave media stalled until ICE consent expires.
    fn check_media_disconnect(self: &Arc<Self>, target: SignalTarget) {
        let Some(transport) = self.media_transport(target) else { return };
        if transport.connected_generation() == 0 {
            return;
        }
        let generation = transport.disconnect_generation();
        let session = Arc::downgrade(self);
        tokio::spawn(async move {
            sleep(Duration::from_secs(3)).await;
            if let Some(session) = session.upgrade() {
                let _ = session.emitter.send(SessionEvent::MediaDisconnected { target, generation });
            }
        });
    }

    async fn on_rtc_event(self: &Arc<Self>, event: RtcEvent) -> EngineResult<()> {`,
    ],
    [
      "                if state == PeerConnectionState::Failed {",
      `                if state == PeerConnectionState::Disconnected {
                    self.check_media_disconnect(target);
                }
                if state == PeerConnectionState::Failed {`,
    ],
  ]);
  patch("mod.rs", [
    [
      "            SessionEvent::Close { source, reason, reconnect_reason, action, retry_now } => {",
      `            SessionEvent::MediaDisconnected { target, generation } => {
                let reason = match target {
                    proto::SignalTarget::Subscriber => proto::ReconnectReason::RrSubscriberFailed,
                    proto::SignalTarget::Publisher => proto::ReconnectReason::RrPublisherFailed,
                };
                self.reconnection_needed(
                    false, false, DisconnectReason::UnknownReason, reason, Some((target, generation)),
                );
            }
            SessionEvent::Close { source, reason, reconnect_reason, action, retry_now } => {`,
    ],
    [
      "                            reason,\n                            reconnect_reason,\n                        );",
      "                            reason,\n                            reconnect_reason,\n                            None,\n                        );",
    ],
    [
      "        reported_reconnect_reason: proto::ReconnectReason,\n    ) {",
      "        reported_reconnect_reason: proto::ReconnectReason,\n        media_disconnect: Option<(proto::SignalTarget, u32)>,\n    ) {",
    ],
    [
      "        if !running_handle.can_reconnect {\n            return;\n        }",
      `        if running_handle.closed || !running_handle.can_reconnect {
            return;
        }

        // Ignore a stale timer after the transport recovered, and let any
        // reconnect already in progress handle a second transport's timer.
        if media_disconnect.is_some_and(|(target, generation)| {
            running_handle.reconnecting
                || !running_handle.session.media_still_disconnected(target, generation)
        }) {
            return;
        }`,
    ],
    [
      "                    proto::ReconnectReason::RrUnknown,\n                );",
      "                    proto::ReconnectReason::RrUnknown,\n                    None,\n                );",
    ],
  ]);
}

if (existsSync(destination)) {
  if (treeHash(destination) !== pin.treeSha256)
    throw new Error(
      "The generated native SDK differs from its pinned patched source. Inspect it before replacing it.",
    );
  console.log("Kept the verified native SDK.");
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
  const patched = original.replace(marker, marker + addition);
  if (hash(patched) !== pin.patchedSourceSha256)
    throw new Error("The native SDK patch checksum did not match the pin.");
  writeFileSync(target, patched);
  // Read-only association used by opt-in local qualification. Never log IDs.
  for (const [file, kind] of [
    ["remote_audio_track.rs", "RtcAudioTrack"],
    ["remote_video_track.rs", "RtcVideoTrack"],
  ]) {
    const path = join(source, "src/room/track", file);
    const original = readFileSync(path, "utf8").replaceAll("\r\n", "\n");
    const marker = `    pub fn rtc_track(&self) -> ${kind} {`;
    if (original.split(marker).length !== 2)
      throw new Error("Native sync association marker changed.");
    const audioGeneration =
      kind === "RtcAudioTrack"
        ? `    /// Native receiver generation, never resolved from a reused SSRC here.
    pub fn native_playback_generation(&self) -> u64 {
        self.transceiver().map_or(0, |t| t.playback_generation())
    }

`
        : "";
    writeFileSync(
      path,
      original.replace(
        marker,
        audioGeneration +
          `    /// PodeTelar qualification adapter: negotiated receiver stream associations.
    pub fn sync_stream_ids(&self) -> Vec<String> {
        self.transceiver().map_or_else(Vec::new, |t| t.receiver().stream_ids())
    }

${marker}`,
      ),
    );
  }
  patchMediaRecovery(source);
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
  console.log("Prepared the verified native SDK.");
}
