// SPDX-License-Identifier: MIT
// Application gain gate at WebRTC's factory mixer seam. Every new source is
// silent before AddSource makes it visible to the audio thread.
#pragma once
#include <atomic>
#include <cmath>
#include <cstdint>
#include <map>
#include <memory>
#include <mutex>
#include "api/audio/audio_mixer.h"
#include "api/make_ref_counted.h"
#include "modules/audio_mixer/audio_mixer_impl.h"

namespace podetelar {
struct MixerEntry {
  uint64_t generation;
  uint32_t ssrc;
  std::atomic<double> gain{0.0};
};

class MixerState {
 public:
  std::mutex mutex;
  std::map<webrtc::AudioMixer::Source*, std::shared_ptr<MixerEntry>> entries;
  uint64_t next_generation = 1;

  uint64_t Capture(uint32_t ssrc) {
    std::lock_guard<std::mutex> lock(mutex);
    uint64_t found = 0;
    for (const auto& [source, entry] : entries) {
      if (entry->ssrc == ssrc) {
        if (found != 0) return 0; // Ambiguous channel: stay silent.
        found = entry->generation;
      }
    }
    return found;
  }
  bool SetGain(uint64_t generation, double gain) {
    if (generation == 0 || !std::isfinite(gain) || gain < 0 || gain > 1) return false;
    std::lock_guard<std::mutex> lock(mutex);
    for (const auto& [source, entry] : entries) {
      if (entry->generation == generation) {
        entry->gain.store(gain);
        return true;
      }
    }
    return false; // Removed/replaced sources cannot be authorized by old tasks.
  }
};

class OutputMixer : public webrtc::AudioMixer {
  class Gate : public Source {
   public:
    Gate(Source* source, std::shared_ptr<MixerEntry> entry)
        : source_(source), entry_(std::move(entry)) {}
    AudioFrameInfo GetAudioFrameWithInfo(int rate, webrtc::AudioFrame* frame) override {
      // Drain the real source, including pre-gain diagnostic taps, while muted.
      const auto info = source_->GetAudioFrameWithInfo(rate, frame);
      const double gain = entry_->gain.load();
      if (gain == 0 || info == AudioFrameInfo::kMuted) {
        frame->Mute();
        return info == AudioFrameInfo::kError ? info : AudioFrameInfo::kMuted;
      }
      if (info == AudioFrameInfo::kNormal && gain < 1) {
        auto* samples = frame->mutable_data();
        for (size_t i = 0; i < frame->samples_per_channel_ * frame->num_channels_; ++i)
          samples[i] = static_cast<int16_t>(std::lround(samples[i] * gain));
      }
      return info;
    }
    int Ssrc() const override { return source_->Ssrc(); }
    int PreferredSampleRate() const override { return source_->PreferredSampleRate(); }
   private:
    Source* source_; // Owned by WebRTC until its matching RemoveSource returns.
    std::shared_ptr<MixerEntry> entry_;
  };
 public:
  OutputMixer() : state_(std::make_shared<MixerState>()), delegate_(webrtc::AudioMixerImpl::Create()) {}
  std::shared_ptr<MixerState> state() const { return state_; }
  bool AddSource(Source* source) override {
    const auto ssrc = static_cast<uint32_t>(source->Ssrc());
    std::lock_guard<std::mutex> lock(state_->mutex);
    if (gates_.contains(source)) return false;
    auto entry = std::make_shared<MixerEntry>();
    entry->generation = state_->next_generation++;
    entry->ssrc = ssrc;
    auto gate = std::make_unique<Gate>(source, entry);
    if (!delegate_->AddSource(gate.get())) return false;
    state_->entries.emplace(source, std::move(entry));
    gates_.emplace(source, std::move(gate));
    return true;
  }
  void RemoveSource(Source* source) override {
    std::lock_guard<std::mutex> lock(state_->mutex);
    const auto gate = gates_.find(source);
    if (gate == gates_.end()) return;
    // Delegate serializes removal against Mix. Only then destroy the wrapper.
    delegate_->RemoveSource(gate->second.get());
    state_->entries.erase(source);
    gates_.erase(gate);
  }
  void Mix(size_t channels, webrtc::AudioFrame* frame) override {
    delegate_->Mix(channels, frame);
  }
 private:
  std::shared_ptr<MixerState> state_;
  std::map<Source*, std::unique_ptr<Gate>> gates_;
  // Destroy the delegate before its source wrappers.
  webrtc::scoped_refptr<webrtc::AudioMixer> delegate_;
};
} // namespace podetelar
