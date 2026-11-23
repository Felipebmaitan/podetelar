// SPDX-License-Identifier: MIT
#pragma once

#include "api/frame_transformer_interface.h"
#include "api/make_ref_counted.h"
#include <memory>
#include <mutex>
#include <set>

namespace podetelar {
// Read only when a receive stream is constructed. An entry lives no longer
// than the wrapper owning the actual enabled receiver cryptor.
class ReceiveCryptors {
public:
  using Transform = webrtc::FrameTransformerInterface;
  static ReceiveCryptors &Instance() {
    static ReceiveCryptors instance;
    return instance;
  }
  void Add(Transform *crypto) {
    std::lock_guard lock(mutex_);
    active_.insert(crypto);
  }
  void Remove(Transform *crypto) {
    std::lock_guard lock(mutex_);
    active_.erase(crypto);
  }
  bool Contains(const Transform *crypto) {
    std::lock_guard lock(mutex_);
    return active_.count(crypto) != 0;
  }

private:
  std::mutex mutex_;
  std::set<const Transform *> active_;
};

// Never hand a frame to a decoder or jitter buffer before crypto attaches.
class DiscardFrames : public webrtc::FrameTransformerInterface {
public:
  void
  Transform(std::unique_ptr<webrtc::TransformableFrameInterface>) override {}
};

template <typename Config> void GuardReceiveConfig(Config &config) {
  if (!ReceiveCryptors::Instance().Contains(config.frame_transformer.get()))
    config.frame_transformer = webrtc::make_ref_counted<DiscardFrames>();
}

} // namespace podetelar
