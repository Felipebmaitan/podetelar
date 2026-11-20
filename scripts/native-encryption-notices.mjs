// SPDX-License-Identifier: MIT
import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export function copyEncryptionSourceNotices(root, notices) {
  const out = join(notices, "source-patches", "native-encryption");
  function copy(source, relative) {
    const target = join(out, relative);
    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(root, source), target);
  }
  for (const file of [
    "src/room/participant/local_participant.rs",
    "src/room/participant/remote_participant.rs",
    "src/room/mod.rs",
    "src/room/e2ee/manager.rs",
    "src/room/publication/remote.rs",
    "src/room/publication/mod.rs",
    "src/room/track/local_video_track.rs",
    "src/room/track/remote_audio_track.rs",
    "src/room/track/remote_video_track.rs",
  ]) {
    copy(`.local/native-sdk/livekit/${file}`, `livekit/${file}`);
  }
  for (const file of [
    "webrtc-sys/src/frame_cryptor.cpp",
    "webrtc-sys/src/peer_connection_factory.cpp",
    "webrtc-sys/src/peer_connection.cpp",
    "webrtc-sys/src/peer_connection.rs",
    "webrtc-sys/include/livekit/peer_connection.h",
    "libwebrtc/src/peer_connection.rs",
    "libwebrtc/src/native/peer_connection.rs",
    "libwebrtc/src/peer_connection_factory.rs",
  ])
    copy(`.local/native-sdk/${file}`, file);
  for (const file of [
    "scripts/setup-native-sdk.mjs",
    "scripts/setup-native-decoder.mjs",
    "scripts/patch-native-output.mjs",
    "scripts/setup-native-audio.mjs",
    "deploy/native-sdk.json",
    "deploy/native-decoder.json",
    "deploy/native-audio.json",
    "apps/publisher/sdk/receiver-publication-tests.rs",
    "apps/publisher/sdk/receive-guard-test.cpp",
    "apps/publisher/decoder/protected_receive_guard.h",
    "apps/publisher/decoder/protected_receive_call.h",
    "apps/publisher/decoder/protected_frame_bounds.h",
  ])
    copy(file, file);
  writeFileSync(
    join(out, "README.txt"),
    "PodeTelar modifies LiveKit 0.10.1, libwebrtc 0.4.0 and webrtc-sys 0.3.53 (Apache-2.0) to enforce native frame encryption before media activation, authenticate the negotiated publication mode and close protected full reconnects before reusing a key. This directory includes the changed LiveKit sources, frame cryptor wrapper, deterministic patch scripts, tests and checksums. Upstream copyright and license notices are retained.\n",
  );
}
