// Copyright (c) 2026 PodeTelar contributors. MIT licensed.
#pragma once
#include <atomic>
#include <cstdint>
namespace podetelar {
// This application injects process-loopback audio. Its SDK adapter permanently
// denies input operations, including when platform output is acquired later.
inline std::atomic<uint64_t> blocked_input_operations{0};
inline std::atomic<uint64_t> platform_output_starts{0};
inline std::atomic<uint64_t> platform_output_stops{0};
inline std::atomic<int32_t> output_delay_ms{-1};
inline std::atomic<uint64_t> mix_calls{0};
inline int32_t BlockInput() { blocked_input_operations.fetch_add(1); return 0; }
}
