// SPDX-License-Identifier: MIT
#include "call/audio_receive_stream.h"
#include "call/video_receive_stream.h"
#include "protected_receive_guard.h"

namespace {
class CountingSink : public webrtc::TransformedFrameCallback {
 public:
  void OnTransformedFrame(
      std::unique_ptr<webrtc::TransformableFrameInterface>) override {
    ++frames;
  }
  size_t frames = 0;
};

template <typename Config>
bool Check(Config& config) {
  auto sink = webrtc::make_ref_counted<CountingSink>();
  podetelar::GuardReceiveConfig(config);
  if (!config.frame_transformer) return false;
  config.frame_transformer->RegisterTransformedFrameCallback(sink);
  config.frame_transformer->RegisterTransformedFrameSinkCallback(sink, 1);
  config.frame_transformer->Transform(nullptr);
  if (sink->frames != 0) return false;
  auto unregistered = config.frame_transformer;
  podetelar::GuardReceiveConfig(config);
  if (config.frame_transformer == unregistered) return false;
  // The production wrapper registers only an already enabled receiver cryptor.
  auto registered = config.frame_transformer;
  podetelar::ReceiveCryptors::Instance().Add(registered.get());
  podetelar::GuardReceiveConfig(config);
  if (config.frame_transformer != registered) return false;
  podetelar::ReceiveCryptors::Instance().Remove(registered.get());
  podetelar::GuardReceiveConfig(config);
  return config.frame_transformer != registered;
}
}  // namespace

int main() {
  webrtc::AudioReceiveStreamInterface::Config audio;
  webrtc::VideoReceiveStreamInterface::Config video(nullptr);
  return Check(audio) && Check(video) ? 0 : 1;
}
