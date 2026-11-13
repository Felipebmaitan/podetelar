// MIT. Deterministic output-only/gain additions to the pinned Apache-2.0 SDK.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export function patchNativeOutput(source, guard) {
  const replace = (path, marker, replacement, expected = 1) => {
    const file = join(source, path),
      original = readFileSync(file, "utf8").replaceAll("\r\n", "\n");
    if (original.split(marker).length !== expected + 1)
      throw new Error(`Native output patch marker changed in ${path}.`);
    writeFileSync(file, original.replaceAll(marker, replacement));
  };
  const method = (name, parameters, body) => {
    const file = join(source, "src/adm_proxy.cpp");
    const original = readFileSync(file, "utf8").replaceAll("\r\n", "\n");
    const pattern = new RegExp(
      `(int32_t|bool|int16_t) AdmProxy::${name}\\(${parameters}\\)( const)? \\{[\\s\\S]*?\\n\\}`,
    );
    const found = original.match(pattern);
    if (!found) throw new Error(`Native input guard marker changed: ${name}`);
    writeFileSync(
      file,
      original.replace(
        pattern,
        `${found[1]} AdmProxy::${name}(${found[0].split("(")[1].split(")")[0]})${found[2] ?? ""} {\n${body}\n}`,
      ),
    );
  };
  writeFileSync(
    join(source, "src/native_output_guard.h"),
    readFileSync(guard, "utf8").replaceAll("\r\n", "\n"),
  );
  replace(
    "src/adm_proxy.cpp",
    '#include "livekit/adm_proxy.h"',
    '#include "livekit/adm_proxy.h"\n#include "native_output_guard.h"',
  );
  replace(
    "src/adm_proxy.cpp",
    `  if (selected_recording_device_.has_value()) {
    if (auto* index = std::get_if<uint16_t>(&*selected_recording_device_)) {
      platform_adm_->SetRecordingDevice(*index);
    } else {
      platform_adm_->SetRecordingDevice(
          std::get<WindowsDeviceType>(*selected_recording_device_));
    }
  }`,
    "  // Recording-device replay intentionally disabled.",
  );
  replace(
    "src/adm_proxy.cpp",
    `  if (selected_stereo_recording_.has_value()) {
    platform_adm_->SetStereoRecording(*selected_stereo_recording_);
  }`,
    "  // Stereo-recording replay intentionally disabled.",
  );
  replace(
    "src/adm_proxy.cpp",
    "void AdmProxy::set_recording_enabled(bool enabled) {\n",
    "void AdmProxy::set_recording_enabled(bool enabled) {\n  if (enabled) podetelar::BlockInput();\n  enabled = false;\n",
  );
  replace(
    "src/adm_proxy.cpp",
    `  // Recording only available through platform ADM when enabled.
  // Synthetic mode doesn't support recording (no microphone).
  if (platform_adm_ && platform_adm_ref_count_ > 0 && recording_enabled_) {
    return platform_adm_.get();
  }
  return nullptr;`,
    "  return nullptr; // PodeTelar never acquires a recording device.",
  );
  for (const parameters of ["uint16_t index", "WindowsDeviceType device"])
    method(
      "SetRecordingDevice",
      parameters,
      "  return podetelar::BlockInput();",
    );
  method(
    "SetStereoRecording",
    "bool enable",
    "  return podetelar::BlockInput();",
  );
  method("InitRecording", "", "  return podetelar::BlockInput();");
  method("StartRecording", "", "  return podetelar::BlockInput();");
  method("InitMicrophone", "", "  return podetelar::BlockInput();");
  method(
    "RecordingIsAvailable",
    "bool\\* available",
    "  *available = false; return 0;",
  );
  method("RecordingIsInitialized", "", "  return false;");
  method("Recording", "", "  return false;");
  method("MicrophoneIsInitialized", "", "  return false;");
  method(
    "StereoRecordingIsAvailable",
    "bool\\* available",
    "  *available = false; return 0;",
  );
  method("StereoRecording", "bool\\* enabled", "  *enabled = false; return 0;");
  // Microphone control queries/setters must not acquire an endpoint interface.
  for (const [name, parameters, body] of [
    ["MicrophoneVolumeIsAvailable", "bool\\* available", "*available = false;"],
    ["SetMicrophoneVolume", "uint32_t volume", "podetelar::BlockInput();"],
    ["MicrophoneVolume", "uint32_t\\* volume", "*volume = 0;"],
    ["MaxMicrophoneVolume", "uint32_t\\* maxVolume", "*maxVolume = 0;"],
    ["MinMicrophoneVolume", "uint32_t\\* minVolume", "*minVolume = 0;"],
    ["MicrophoneMuteIsAvailable", "bool\\* available", "*available = false;"],
    ["SetMicrophoneMute", "bool enable", "podetelar::BlockInput();"],
    ["MicrophoneMute", "bool\\* enabled", "*enabled = false;"],
  ])
    method(name, parameters, `  ${body} return 0;`);
  replace(
    "src/adm_proxy.cpp",
    "      return platform_adm_->PlayoutDelay(delayMS);",
    "      auto result = platform_adm_->PlayoutDelay(delayMS);\n      podetelar::output_delay_ms.store(result == 0 ? *delayMS : -1);\n      return result;",
  );
  replace(
    "src/adm_proxy.cpp",
    "      platform_adm_->StartPlayout();",
    "      if (platform_adm_->StartPlayout() == 0) podetelar::platform_output_starts.fetch_add(1);",
  );
  replace(
    "src/adm_proxy.cpp",
    "      return platform_adm_->StartPlayout();",
    "      auto result = platform_adm_->StartPlayout();\n      if (result == 0) podetelar::platform_output_starts.fetch_add(1);\n      return result;",
  );
  replace(
    "src/adm_proxy.cpp",
    "      platform_adm_->StopPlayout();",
    "      platform_adm_->StopPlayout();\n      podetelar::platform_output_stops.fetch_add(1);\n      podetelar::output_delay_ms.store(-1);",
    2,
  );
  replace(
    "src/synthetic_audio_device.cpp",
    "int32_t SyntheticAudioDevice::PlayoutDelay(uint16_t* delayMS) const {\n  return 0;",
    "int32_t SyntheticAudioDevice::PlayoutDelay(uint16_t* delayMS) const {\n  *delayMS = 0; // Synthetic output has no physical device estimate.\n  return 0;",
  );
  // Mixer admission is closed before a source is visible on the audio thread.
  writeFileSync(
    join(source, "src/native_output_mixer.h"),
    readFileSync(
      join(dirname(guard), "native_output_mixer.h"),
      "utf8",
    ).replaceAll("\r\n", "\n"),
  );
  replace(
    "include/livekit/webrtc.h",
    "namespace livekit_ffi {\nclass RtcRuntime;",
    "namespace podetelar { class MixerState; }\nnamespace livekit_ffi {\nclass RtcRuntime;",
  );
  replace(
    "include/livekit/webrtc.h",
    "class MediaStreamTrack;",
    "class AudioDeviceController;\nclass MediaStreamTrack;",
  );
  replace(
    "include/livekit/webrtc.h",
    "  webrtc::Thread* network_thread() const;",
    `  void set_audio_device(std::shared_ptr<AudioDeviceController> value) {
    std::lock_guard<std::mutex> lock(output_mutex_); audio_device_ = value;
  }
  std::shared_ptr<AudioDeviceController> audio_device() {
    std::lock_guard<std::mutex> lock(output_mutex_); return audio_device_.lock();
  }
  void set_output_mixer(std::shared_ptr<podetelar::MixerState> value) { output_mixer_ = value; }
  std::shared_ptr<podetelar::MixerState> output_mixer() const { return output_mixer_; }
  webrtc::Thread* network_thread() const;`,
  );
  replace(
    "include/livekit/webrtc.h",
    "  RtcRuntime();",
    "  RtcRuntime();\n  std::mutex output_mutex_;\n  std::weak_ptr<AudioDeviceController> audio_device_;\n  std::shared_ptr<podetelar::MixerState> output_mixer_;",
  );
  replace(
    "include/livekit/webrtc.h",
    "#include <memory>",
    "#include <memory>\n#include <mutex>",
  );
  replace(
    "src/peer_connection_factory.cpp",
    '#include "livekit/peer_connection_factory.h"',
    '#include "livekit/peer_connection_factory.h"\n#include "native_output_mixer.h"',
  );
  replace(
    "src/peer_connection_factory.cpp",
    "  audio_device_ =\n      std::make_shared<AudioDeviceController>(rtc_runtime_, adm_proxy_);",
    "  audio_device_ =\n      std::make_shared<AudioDeviceController>(rtc_runtime_, adm_proxy_);\n  rtc_runtime_->set_audio_device(audio_device_);\n  auto mixer = webrtc::make_ref_counted<podetelar::OutputMixer>();\n  rtc_runtime_->set_output_mixer(mixer->state());\n  dependencies.audio_mixer = mixer;",
  );
  replace(
    "include/livekit/audio_track.h",
    "  void add_sink(",
    `  std::shared_ptr<AudioDeviceController> output_device() const { return rtc_runtime_->audio_device(); }
  bool set_playback_volume(uint64_t generation, double volume) const;
  void add_sink(`,
  );
  replace(
    "src/audio_track.rs",
    "        type AudioTrack;",
    "        type AudioDeviceController = crate::audio_device_controller::ffi::AudioDeviceController;\n        type AudioTrack;",
  );
  replace(
    "src/audio_track.rs",
    "        fn add_sink(",
    "        fn output_device(self: &AudioTrack) -> SharedPtr<AudioDeviceController>;\n        fn set_playback_volume(self: &AudioTrack, generation: u64, volume: f64) -> bool;\n        fn add_sink(",
  );
  replace(
    "src/audio_track.cpp",
    "#include <iterator>",
    '#include <iterator>\n#include "native_output_mixer.h"',
  );
  replace(
    "src/audio_track.cpp",
    "AudioTrack::~AudioTrack() {",
    `bool AudioTrack::set_playback_volume(uint64_t generation, double volume) const {
  auto mixer = rtc_runtime_->output_mixer();
  return mixer && mixer->SetGain(generation, volume);
}

AudioTrack::~AudioTrack() {`,
  );
  replace(
    "include/livekit/rtp_transceiver.h",
    "  MediaType media_type() const;",
    "  uint64_t playback_generation() const { return playback_generation_; }\n  MediaType media_type() const;",
  );
  replace(
    "include/livekit/rtp_transceiver.h",
    "  std::shared_ptr<RtcRuntime> rtc_runtime_;",
    "  uint64_t playback_generation_ = 0; // Immutable after construction/event dispatch.\n  std::shared_ptr<RtcRuntime> rtc_runtime_;",
  );
  replace(
    "src/rtp_transceiver.rs",
    "        fn media_type(",
    "        fn playback_generation(self: &RtpTransceiver) -> u64;\n        fn media_type(",
  );
  replace(
    "src/rtp_transceiver.cpp",
    '#include "livekit/rtp_transceiver.h"',
    '#include "livekit/rtp_transceiver.h"\n#include "native_output_mixer.h"',
  );
  replace(
    "src/rtp_transceiver.cpp",
    "      peer_connection_(std::move(peer_connection)) {}",
    `      peer_connection_(std::move(peer_connection)) {
  // Capture at the native event boundary, before deferred Rust event handling.
  auto receiver = transceiver_->receiver();
  auto parameters = receiver->GetParameters();
  auto mixer = rtc_runtime_->output_mixer();
  if (mixer && receiver->media_type() == webrtc::MediaType::AUDIO && parameters.encodings.size() == 1 && parameters.encodings[0].ssrc.has_value()) {
    playback_generation_ = mixer->Capture(*parameters.encodings[0].ssrc);
  }
}`,
  );
  replace(
    "src/audio_device_controller.rs",
    "pub mod ffi {",
    "pub mod ffi {\n    pub struct OutputOnlyAudit {\n        pub blocked_input_operations: u64,\n        pub platform_output_starts: u64,\n        pub platform_output_stops: u64,\n        pub output_delay_ms: i32,\n    }",
  );
  replace(
    "src/audio_device_controller.rs",
    "        fn audio_device(",
    "        fn output_only_audit() -> OutputOnlyAudit;\n        fn audio_device(",
  );
  replace(
    "include/livekit/audio_device_controller.h",
    "class RtcRuntime;",
    "class RtcRuntime;\nstruct OutputOnlyAudit;\nOutputOnlyAudit output_only_audit();",
  );
  replace(
    "src/audio_device_controller.cpp",
    "#include <string>",
    '#include <string>\n#include "native_output_guard.h"\n#include "webrtc-sys/src/audio_device_controller.rs.h"',
  );
  replace(
    "src/audio_device_controller.cpp",
    "namespace livekit_ffi {",
    "namespace livekit_ffi {\nOutputOnlyAudit output_only_audit() {\n  return OutputOnlyAudit{podetelar::blocked_input_operations.load(), podetelar::platform_output_starts.load(), podetelar::platform_output_stops.load(), podetelar::output_delay_ms.load()};\n}",
  );
}
