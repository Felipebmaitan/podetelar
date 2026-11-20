// SPDX-License-Identifier: MIT
#pragma once

#include <algorithm>
#include <array>
#include <cstdint>
#include <span>
#include <vector>

#include "common_video/h264/h264_common.h"
#ifdef RTC_ENABLE_H265
#include "common_video/h265/h265_common.h"
#endif

namespace podetelar {

// Spans borrow the input or this instance's decoded buffer. Keep both alive
// through AEAD open. The clear codec prefix is supplied separately as AAD.
struct ProtectedFramePayload {
  std::vector<uint8_t> decoded;
  std::span<const uint8_t> ciphertext;
  std::span<const uint8_t> nonce;
  uint8_t key_index = 0;

  bool Parse(std::span<const uint8_t> body, bool h264, bool h265) {
    constexpr std::array<uint8_t, 3> escape = {0, 0, 3};
    if ((h264 || h265) &&
        std::search(body.begin(), body.end(), escape.begin(), escape.end()) !=
            body.end()) {
#ifdef RTC_ENABLE_H265
      if (h265)
        decoded = webrtc::H265::ParseRbsp(body.data(), body.size());
      else
#endif
        decoded = webrtc::H264::ParseRbsp(body.data(), body.size());
      body = decoded;
    }
    constexpr size_t nonce_size = 12, trailer_size = 2, tag_size = 16;
    if (body.size() < tag_size + nonce_size + trailer_size ||
        body[body.size() - trailer_size] != nonce_size)
      return false;
    key_index = body.back();
    const size_t cipher_size = body.size() - nonce_size - trailer_size;
    ciphertext = body.first(cipher_size);
    nonce = body.subspan(cipher_size, nonce_size);
    return true;
  }
};
}  // namespace podetelar
