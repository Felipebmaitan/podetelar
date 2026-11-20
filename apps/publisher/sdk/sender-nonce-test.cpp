// SPDX-License-Identifier: MIT
#include "sender_nonce.h"

#include <algorithm>
#include <chrono>
#include <iostream>
#include <set>
#include <thread>
#include <type_traits>
#include <vector>

namespace {
using podetelar::SenderNonce;
using podetelar::SenderNonceAllocator;
using podetelar::SenderNonceCursor;

int ZeroRandom(uint8_t *bytes, size_t size) {
  std::fill(bytes, bytes + size, 0);
  return 1;
}
int HighRandom(uint8_t *bytes, size_t size) {
  std::fill(bytes, bytes + size, 254);
  return 1;
}
int RejectedRandom(uint8_t *bytes, size_t size) {
  std::fill(bytes, bytes + size, 255);
  return 1;
}
int FailedRandom(uint8_t *, size_t) { return 0; }
size_t retry_calls = 0;
int RetriedRandom(uint8_t *bytes, size_t size) {
  std::fill(bytes, bytes + size, retry_calls++ == 0 ? 255 : 0);
  return 1;
}

bool Nonzero(const SenderNonce &nonce) {
  return std::all_of(nonce.begin(), nonce.end(),
                     [](uint8_t b) { return b != 0; });
}

bool Boundaries() {
  static_assert(!std::is_copy_constructible_v<SenderNonceCursor>);
  static_assert(!std::is_move_constructible_v<SenderNonceCursor>);
  const uint64_t maximum = SenderNonceAllocator::kLimit;
  SenderNonceAllocator source(ZeroRandom, maximum, maximum - 5);
  SenderNonceCursor cursor(source);
  std::set<SenderNonce> seen;
  SenderNonce nonce{};
  for (size_t i = 0; i < 5; ++i) {
    if (!cursor.Next(nonce) || !Nonzero(nonce) || !seen.insert(nonce).second)
      return false;
  }
  const auto previous = nonce;
  if (cursor.Next(nonce) || nonce != previous)
    return false;
  SenderNonceCursor later(source);
  if (later.Next(nonce))
    return false;

  // Offset addition must wrap in the 12-digit domain without duplicating.
  SenderNonceAllocator wrapping(HighRandom, 3);
  SenderNonceCursor wrapped(wrapping);
  const SenderNonce high = {255, 255, 255, 255, 255, 255,
                            255, 255, 255, 255, 255, 255};
  SenderNonce low{};
  low.fill(1);
  if (!wrapped.Next(nonce) || nonce != high || !wrapped.Next(nonce) ||
      nonce != low)
    return false;
  if (!wrapped.Next(nonce) || !Nonzero(nonce) || nonce == low || nonce == high)
    return false;
  return !wrapped.Next(nonce);
}

bool FailClosed() {
  for (auto random : {FailedRandom, RejectedRandom}) {
    SenderNonceAllocator source(random);
    SenderNonceCursor cursor(source);
    SenderNonce nonce{};
    nonce.fill(19);
    const auto previous = nonce;
    if (cursor.Next(nonce) || cursor.Next(nonce) || nonce != previous)
      return false;
  }
  SenderNonceAllocator retried(RetriedRandom);
  SenderNonceCursor cursor(retried);
  SenderNonce nonce{};
  if (retry_calls != 2 || !cursor.Next(nonce) || !Nonzero(nonce))
    return false;
  return true;
}

bool Recreation() {
  SenderNonceAllocator source(ZeroRandom);
  std::set<SenderNonce> seen;
  for (size_t generation = 0; generation < 1000; ++generation) {
    // Models alternating media/data workers and cryptor recreation. Abandoned
    // block tails must never be handed to a replacement worker.
    SenderNonceCursor cursor(source);
    SenderNonce nonce{};
    if (!cursor.Next(nonce) || !Nonzero(nonce) || !seen.insert(nonce).second)
      return false;
  }
  return true;
}

bool Concurrency(bool exhausted) {
  constexpr size_t workers = 12;
  constexpr size_t per_worker = 20000;
  constexpr uint64_t limit = 65539; // Partial final block under contention.
  SenderNonceAllocator source(ZeroRandom, exhausted ? limit : 1000000);
  std::array<std::vector<SenderNonce>, workers> outputs;
  std::vector<std::thread> threads;
  std::atomic<size_t> ready{0};
  std::atomic<bool> start{false};
  for (size_t worker = 0; worker < workers; ++worker) {
    threads.emplace_back([&, worker] {
      SenderNonceCursor cursor(source);
      auto &output = outputs[worker];
      output.reserve(per_worker);
      ready.fetch_add(1, std::memory_order_release);
      while (!start.load(std::memory_order_acquire))
        std::this_thread::yield();
      SenderNonce nonce{};
      while ((exhausted || output.size() < per_worker) && cursor.Next(nonce))
        output.push_back(nonce);
    });
  }
  while (ready.load(std::memory_order_acquire) != workers)
    std::this_thread::yield();
  start.store(true, std::memory_order_release);
  for (auto &thread : threads)
    thread.join();
  std::vector<SenderNonce> combined;
  for (const auto &output : outputs) {
    if (!exhausted && output.size() != per_worker)
      return false;
    combined.insert(combined.end(), output.begin(), output.end());
  }
  if (combined.size() != (exhausted ? limit : workers * per_worker))
    return false;
  std::sort(combined.begin(), combined.end());
  return std::adjacent_find(combined.begin(), combined.end()) ==
             combined.end() &&
         std::all_of(combined.begin(), combined.end(), Nonzero);
}

bool ProductionAndTiming() {
  SenderNonceAllocator source(ZeroRandom);
  SenderNonceCursor cursor(source);
  constexpr size_t count = 1000000;
  uint64_t checksum = 0;
  SenderNonce nonce{};
  const auto begin = std::chrono::steady_clock::now();
  for (size_t i = 0; i < count; ++i) {
    if (!cursor.Next(nonce))
      return false;
    for (uint8_t byte : nonce)
      checksum += byte;
  }
  const auto elapsed = std::chrono::steady_clock::now() - begin;
  std::cout
      << "nonce allocator: "
      << std::chrono::duration_cast<std::chrono::nanoseconds>(elapsed).count() /
             static_cast<double>(count)
      << " ns/nonce; checksum=" << checksum << '\n';
  SenderNonce first{}, second{};
  return podetelar::NextSenderNonce(first) &&
         podetelar::NextSenderNonce(second) && first != second &&
         Nonzero(first) && Nonzero(second);
}
} // namespace

int main() {
  if (!Boundaries() || !FailClosed() || !Recreation() || !Concurrency(false) ||
      !Concurrency(true) || !ProductionAndTiming()) {
    std::cerr << "Sender nonce regression failed.\n";
    return 1;
  }
  std::cout << "Sender nonce concurrency, recreation, overflow and RNG checks "
               "passed.\n";
  return 0;
}
