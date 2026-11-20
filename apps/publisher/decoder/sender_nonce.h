// SPDX-License-Identifier: MIT
#pragma once

#include <openssl/rand.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <limits>

namespace podetelar {

using SenderNonce = std::array<uint8_t, 12>;

// One instance is shared by every media and data encryptor in this process.
// Fresh processes/connections must use independently generated sender keys.
// Reservations never wrap; unused reservations are deliberately abandoned.
class SenderNonceAllocator {
public:
  using RandomFill = int (*)(uint8_t *, size_t);
  static constexpr uint64_t kBlockSize = 4096;
  static constexpr uint64_t kLimit = std::numeric_limits<uint64_t>::max();

  // The bounds and random source are injectable for deterministic regression
  // checks. Production only uses the process-lifetime singleton below.
  explicit SenderNonceAllocator(RandomFill random_fill = RAND_bytes,
                                uint64_t limit = kLimit,
                                uint64_t initial_counter = 0)
      : next_(initial_counter), limit_(limit) {
    size_t filled = 0;
    for (size_t attempt = 0; filled < offset_.size() && attempt < 32;
         ++attempt) {
      SenderNonce random{};
      if (!random_fill || random_fill(random.data(), random.size()) != 1)
        return;
      for (uint8_t digit : random) {
        // Rejection sampling gives uniform base-255 offset digits.
        if (digit != 255 && filled < offset_.size())
          offset_[filled++] = digit;
      }
    }
    ready_ = filled == offset_.size();
  }

  SenderNonceAllocator(const SenderNonceAllocator &) = delete;
  SenderNonceAllocator &operator=(const SenderNonceAllocator &) = delete;

  bool Reserve(uint64_t &begin, uint64_t &end) {
    if (!ready_)
      return false;
    uint64_t first = next_.load(std::memory_order_relaxed);
    while (first < limit_) {
      const uint64_t after = first + std::min(kBlockSize, limit_ - first);
      if (next_.compare_exchange_weak(first, after,
                                      std::memory_order_relaxed)) {
        begin = first;
        end = after;
        return true;
      }
    }
    return false;
  }

  void Encode(uint64_t counter, SenderNonce &nonce) const {
    unsigned carry = 0;
    for (size_t i = nonce.size(); i-- > 0;) {
      const unsigned digit = static_cast<unsigned>(counter % 255);
      counter /= 255;
      const unsigned sum = offset_[i] + digit + carry;
      nonce[i] = static_cast<uint8_t>(sum % 255 + 1);
      carry = sum / 255;
    }
    // Addition modulo 255^12 is bijective, and 255^12 exceeds the entire
    // uint64 counter space. Bytes 1..255 never need H.26x emulation escaping.
  }

private:
  std::atomic<uint64_t> next_;
  const uint64_t limit_;
  SenderNonce offset_{};
  bool ready_ = false;
};

class SenderNonceCursor {
public:
  explicit SenderNonceCursor(SenderNonceAllocator &allocator)
      : allocator_(allocator) {}
  SenderNonceCursor(const SenderNonceCursor &) = delete;
  SenderNonceCursor &operator=(const SenderNonceCursor &) = delete;

  bool Next(SenderNonce &nonce) {
    if (next_ == end_ && !allocator_.Reserve(next_, end_))
      return false;
    allocator_.Encode(next_++, nonce);
    return true;
  }

private:
  SenderNonceAllocator &allocator_;
  uint64_t next_ = 0;
  uint64_t end_ = 0;
};

inline bool NextSenderNonce(SenderNonce &nonce) {
  static SenderNonceAllocator allocator;
  thread_local SenderNonceCursor cursor(allocator);
  return cursor.Next(nonce);
}

} // namespace podetelar
