// SPDX-License-Identifier: MIT
#include <openssl/aead.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <span>
#include <vector>

#include "protected_frame_payload.h"
#include "rtc_base/buffer.h"

namespace {
constexpr std::array<uint8_t, 6> aad{0, 0, 1, 101, 0, 100};
constexpr std::array<uint8_t, 12> iv{1, 2, 3, 4, 0, 0, 1, 0, 0, 3, 5, 6};
constexpr std::array<uint8_t, 10> plain{228, 201, 111, 91, 89, 156, 134, 153, 18, 66};
// Public AES-128-GCM test vector under the all-zero test key. Emulation
// prevention inserts bytes inside the ciphertext, at the tag boundary and
// twice inside this older sender's IV.
constexpr std::array<uint8_t, 26> sealed{
    4, 0, 0, 3, 9, 10, 21, 32, 0, 0, 2, 74, 224, 89, 73, 105,
    124, 35, 94, 67, 250, 78, 238, 204, 16, 37};

bool Open(const podetelar::ProtectedFramePayload& envelope,
          std::span<const uint8_t> header) {
  bssl::ScopedEVP_AEAD_CTX ctx;
  constexpr std::array<uint8_t, 16> public_test_key{};
  if (!EVP_AEAD_CTX_init(ctx.get(), EVP_aead_aes_128_gcm(), public_test_key.data(),
                         public_test_key.size(), 16, nullptr))
    return false;
  std::array<uint8_t, 32> opened{};
  size_t size = 0;
  return EVP_AEAD_CTX_open(ctx.get(), opened.data(), &size, opened.size(),
                           envelope.nonce.data(), envelope.nonce.size(),
                           envelope.ciphertext.data(), envelope.ciphertext.size(),
                           header.data(), header.size()) == 1 &&
         std::ranges::equal(std::span(opened).first(size), plain);
}

bool CheckCodec(bool hevc) {
  webrtc::Buffer body;
  body.AppendData(sealed);
  body.AppendData(iv);
  constexpr std::array<uint8_t, 2> trailer{12, 3};
  body.AppendData(trailer);
  webrtc::Buffer wire;
  if (hevc)
    webrtc::H265::WriteRbsp(body.data(), body.size(), &wire);
  else
    webrtc::H264::WriteRbsp(body.data(), body.size(), &wire);
  if (wire.size() != body.size() + 4) return false;

  podetelar::ProtectedFramePayload envelope;
  if (!envelope.Parse(wire, !hevc, hevc) || envelope.key_index != 3 ||
      !std::ranges::equal(envelope.ciphertext, sealed) ||
      !std::ranges::equal(envelope.nonce, iv) || !Open(envelope, aad))
    return false;
  auto changed_aad = aad;
  changed_aad.back() ^= 1;
  if (Open(envelope, changed_aad)) return false;

  wire[0] ^= 1;
  if (!envelope.Parse(wire, !hevc, hevc) || Open(envelope, aad)) return false;
  // The wire length appears sufficient until the emulation byte is removed.
  std::array<uint8_t, 30> short_body{};
  short_body[2] = 3;
  short_body[28] = 12;
  if (envelope.Parse(short_body, !hevc, hevc)) return false;
  short_body[2] = 8;
  short_body[28] = 8;
  return !envelope.Parse(short_body, !hevc, hevc);
}
}  // namespace

int main() { return CheckCodec(false) && CheckCodec(true) ? 0 : 1; }
