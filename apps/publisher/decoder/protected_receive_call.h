// SPDX-License-Identifier: MIT
#pragma once

#include "api/field_trials_view.h"
#include "api/peer_connection_interface.h"
#include "pc/media_factory.h"
#include "protected_receive_guard.h"
#include <atomic>
#include <map>
#include <memory>
#include <utility>

namespace podetelar {
// Each protected peer gets an independent local policy carried by its WebRTC
// environment. Register all view copies so losing the marker fails construction
// instead of silently creating an unguarded connection. Remote SDP cannot set
// it.
class ProtectedTrials final : public webrtc::FieldTrialsView {
public:
  using Receipt = std::shared_ptr<std::atomic_bool>;

  ProtectedTrials(std::unique_ptr<webrtc::FieldTrialsView> base,
                  Receipt receipt)
      : base_(std::move(base)), receipt_(std::move(receipt)) {
    auto &registry = Registry();
    std::lock_guard lock(registry.mutex);
    registry.receipts.emplace(this, receipt_);
  }
  ~ProtectedTrials() override {
    auto &registry = Registry();
    std::lock_guard lock(registry.mutex);
    registry.receipts.erase(this);
  }
  std::string Lookup(absl::string_view key) const override {
    return base_->Lookup(key);
  }
  std::unique_ptr<webrtc::FieldTrialsView> CreateCopy() const override {
    return std::make_unique<ProtectedTrials>(base_->CreateCopy(), receipt_);
  }
  static Receipt ReceiptFor(const webrtc::FieldTrialsView &trials) {
    auto &registry = Registry();
    std::lock_guard lock(registry.mutex);
    auto it = registry.receipts.find(&trials);
    return it == registry.receipts.end() ? nullptr : it->second;
  }

private:
  struct Views {
    std::mutex mutex;
    std::map<const webrtc::FieldTrialsView *, Receipt> receipts;
  };
  static Views &Registry() {
    static Views views;
    return views;
  }
  std::unique_ptr<webrtc::FieldTrialsView> base_;
  Receipt receipt_;
};

namespace call_adapter {
using namespace webrtc;

// The underlying Call still owns and returns the original streams. Only the
// creation of receive streams needs interception. No packet enters this class.
class ProtectedCall final : public Call {
public:
  explicit ProtectedCall(std::unique_ptr<Call> call) : call_(std::move(call)) {}
  AudioReceiveStreamInterface *CreateAudioReceiveStream(
      const AudioReceiveStreamInterface::Config &original) override {
    auto config = original;
    GuardReceiveConfig(config);
    return call_->CreateAudioReceiveStream(config);
  }
  VideoReceiveStreamInterface *CreateVideoReceiveStream(
      VideoReceiveStreamInterface::Config config) override {
    GuardReceiveConfig(config);
    return call_->CreateVideoReceiveStream(std::move(config));
  }
  AudioSendStream *
  CreateAudioSendStream(const AudioSendStream::Config &config) override {
    return call_->CreateAudioSendStream(config);
  }
  VideoSendStream *
  CreateVideoSendStream(VideoSendStream::Config config,
                        VideoEncoderConfig encoder,
                        EncoderSwitchRequestCallback callback) override {
    return call_->CreateVideoSendStream(std::move(config), std::move(encoder),
                                        std::move(callback));
  }
  VideoSendStream *
  CreateVideoSendStream(VideoSendStream::Config config,
                        VideoEncoderConfig encoder,
                        EncoderSwitchRequestCallback callback,
                        std::unique_ptr<FecController> fec) override {
    return call_->CreateVideoSendStream(std::move(config), std::move(encoder),
                                        std::move(callback), std::move(fec));
  }
  void DestroyAudioSendStream(AudioSendStream *stream) override {
    call_->DestroyAudioSendStream(stream);
  }
  void DestroyAudioReceiveStream(AudioReceiveStreamInterface *stream) override {
    call_->DestroyAudioReceiveStream(stream);
  }
  void DestroyVideoSendStream(VideoSendStream *stream) override {
    call_->DestroyVideoSendStream(stream);
  }
  void DestroyVideoReceiveStream(VideoReceiveStreamInterface *stream) override {
    call_->DestroyVideoReceiveStream(stream);
  }
  FlexfecReceiveStream *CreateFlexfecReceiveStream(
      const FlexfecReceiveStream::Config config) override {
    return call_->CreateFlexfecReceiveStream(config);
  }
  void DestroyFlexfecReceiveStream(FlexfecReceiveStream *stream) override {
    call_->DestroyFlexfecReceiveStream(stream);
  }
  void AddAdaptationResource(scoped_refptr<Resource> resource) override {
    call_->AddAdaptationResource(std::move(resource));
  }
  PacketReceiver *Receiver() override { return call_->Receiver(); }
  RtpTransportControllerSendInterface *GetTransportControllerSend() override {
    return call_->GetTransportControllerSend();
  }
  Stats GetStats() const override { return call_->GetStats(); }
  void SignalChannelNetworkState(MediaType media, NetworkState state) override {
    call_->SignalChannelNetworkState(media, state);
  }
  void OnAudioTransportOverheadChanged(int overhead) override {
    call_->OnAudioTransportOverheadChanged(overhead);
  }
  void OnUpdateSyncGroup(AudioReceiveStreamInterface &stream,
                         absl::string_view group) override {
    call_->OnUpdateSyncGroup(stream, group);
  }
  void OnSentPacket(const SentPacketInfo &packet) override {
    call_->OnSentPacket(packet);
  }
  void
  SetClientBitratePreferences(const BitrateSettings &preferences) override {
    call_->SetClientBitratePreferences(preferences);
  }
  void SetPreferredRtcpCcAckType(RtcpFeedbackType type) override {
    call_->SetPreferredRtcpCcAckType(type);
  }
  std::optional<int> FeedbackAccordingToRfc8888Count() override {
    return call_->FeedbackAccordingToRfc8888Count();
  }
  std::optional<int> FeedbackAccordingToTransportCcCount() override {
    return call_->FeedbackAccordingToTransportCcCount();
  }
  void DisconnectFromNetworkThread() override {
    call_->DisconnectFromNetworkThread();
  }
  TaskQueueBase *network_thread() const override {
    return call_->network_thread();
  }
  TaskQueueBase *worker_thread() const override {
    return call_->worker_thread();
  }

private:
  std::unique_ptr<Call> call_;
};
} // namespace call_adapter

class ProtectedMediaFactory final : public webrtc::MediaFactory {
public:
  explicit ProtectedMediaFactory(std::unique_ptr<webrtc::MediaFactory> original)
      : original_(std::move(original)) {}
  std::unique_ptr<webrtc::MediaEngineInterface>
  CreateMediaEngine(const webrtc::Environment &env,
                    webrtc::PeerConnectionFactoryDependencies &deps) override {
    return original_->CreateMediaEngine(env, deps);
  }
  std::unique_ptr<webrtc::Call> CreateCall(webrtc::CallConfig config) override {
    auto receipt = ProtectedTrials::ReceiptFor(config.env.field_trials());
    auto call = original_->CreateCall(std::move(config));
    if (receipt && call) {
      auto guarded =
          std::make_unique<call_adapter::ProtectedCall>(std::move(call));
      receipt->store(true);
      return guarded;
    }
    return call;
  }

private:
  std::unique_ptr<webrtc::MediaFactory> original_;
};
} // namespace podetelar
