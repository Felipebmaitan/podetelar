// Copyright (c) 2026 PodeTelar contributors. MIT licensed.
#pragma once

#include <array>
#include <cstdint>
#include <memory>
#include <span>
#include <string>
#include <utility>

#include "api/crypto/frame_crypto_transformer.h"
#include "common_video/h264/h264_common.h"
#include "modules/rtp_rtcp/source/rtp_video_header.h"

namespace podetelar {
namespace frame_bounds {
// AES-GCM in the pinned transformer needs a 16-byte tag, 12-byte IV and
// 2-byte trailer after the codec's clear header and after H.26x unescaping.
inline constexpr size_t kMinimumTail = 16 + 12 + 2;

constexpr bool HasCompleteTail(std::span<const uint8_t> tail, bool escaped) {
  if (tail.size() < kMinimumTail) return false;
  // ParseRbsp removes at most one byte from each group of three. This fast
  // path needs no second scan or copy of ordinary compressed frames.
  if (!escaped || tail.size() >= kMinimumTail * 3 / 2) return true;
  size_t decoded = tail.size();
  for (size_t i = 0; i + 2 < tail.size(); ++i) {
    if (tail[i] == 0 && tail[i + 1] == 0 && tail[i + 2] == 3) {
      --decoded;
      i += 2;
    }
  }
  return decoded >= kMinimumTail;
}

// Boundary cases use synthetic lengths only. They run whenever a native build
// compiles this adapter and impose no per-frame test cost.
static_assert([] {
  std::array<uint8_t, kMinimumTail + 1> bytes{};
  const std::span<const uint8_t> view(bytes);
  if (!HasCompleteTail(view.first(kMinimumTail), false) ||
      HasCompleteTail(view.first(kMinimumTail - 1), false)) return false;
  bytes[2] = 3;  // one emulation-prevention byte
  return HasCompleteTail(view, true) && !HasCompleteTail(view.first(kMinimumTail), true);
}());

// The clear header length in webrtc-sdk/webrtc a545de4 is a uint8_t. This
// helper is only necessary for small H.26x frames. Use the same WebRTC Annex B
// indexer for both codecs (H265::FindNaluIndices delegates to H264's).
inline uint8_t SmallH26xHeader(std::span<const uint8_t> data, bool hevc) {
  for (const auto& nal : webrtc::H264::FindNaluIndices(data)) {
    const size_t offset = nal.payload_start_offset;
    if (offset >= data.size()) continue;
    const uint8_t type = hevc ? ((data[offset] >> 1) & 0x3f) : (data[offset] & 0x1f);
    // H.264 non-IDR/IDR slices, or H.265 TRAIL through RASL and BLA through CRA.
    const bool slice = hevc ? (type <= 9 || (type >= 16 && type <= 21))
                            : (type == 1 || type == 5);
    if (slice) return static_cast<uint8_t>(offset + 2);
  }
  return 0;
}

inline bool CanDecrypt(webrtc::TransformableFrameInterface& frame,
                       webrtc::FrameCryptorTransformer::MediaType media) {
  const std::span<const uint8_t> data = frame.GetData();
  if (data.size() < kMinimumTail || data[data.size() - 2] != 12) return false;
  if (data.size() >= UINT8_MAX + kMinimumTail * 3 / 2) return true;

  using Type = webrtc::FrameCryptorTransformer::MediaType;
  if (media == Type::kAudioFrame) return HasCompleteTail(data.subspan(1), false);
  auto& video = static_cast<webrtc::TransformableVideoFrameInterface&>(frame);
  const auto codec = video.header().codec;
  const bool hevc = codec == webrtc::VideoCodecType::kVideoCodecH265;
  const bool h26x = hevc || codec == webrtc::VideoCodecType::kVideoCodecH264;
  size_t header = 0;
  if (h26x) {
    header = SmallH26xHeader(data, hevc);
  } else if (codec == webrtc::VideoCodecType::kVideoCodecVP8) {
    header = video.IsKeyFrame() ? 10 : 3;
  }
  return header <= data.size() && HasCompleteTail(data.subspan(header), h26x);
}
}  // namespace frame_bounds

// The prebuilt transformer parses frames on its worker queue. Validate the
// same owned frame before scheduling it. No frame is rewritten.
class ProtectedReceiverCryptor final : public webrtc::FrameCryptorTransformer {
 public:
  ProtectedReceiverCryptor(webrtc::Thread* thread, const std::string& identity,
                           MediaType media, Algorithm algorithm,
                           webrtc::scoped_refptr<webrtc::KeyProvider> keys)
      : FrameCryptorTransformer(thread, identity, media, algorithm, std::move(keys)),
        media_(media), gcm_(algorithm == Algorithm::kAesGcm) {}

  void Transform(std::unique_ptr<webrtc::TransformableFrameInterface> frame) override {
    if (gcm_ && frame &&
        frame->GetDirection() == webrtc::TransformableFrameInterface::Direction::kReceiver &&
        frame_bounds::CanDecrypt(*frame, media_)) {
      FrameCryptorTransformer::Transform(std::move(frame));
    }
  }

 private:
  const MediaType media_;
  const bool gcm_;
};
}  // namespace podetelar
