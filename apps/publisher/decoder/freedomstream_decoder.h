// Copyright (c) 2026 FreedomStream contributors. MIT licensed.
// Native WebRTC decoder adapter for the separately licensed bundled GStreamer.
#pragma once

#include <atomic>
#include <map>
#include <mutex>
#include <gst/app/gstappsrc.h>
#include <gst/app/gstappsink.h>
#include <gst/video/video.h>
#include "api/video/i420_buffer.h"
#include "api/video/video_frame.h"
#include "modules/video_coding/include/video_error_codes.h"
#include "libyuv/planar_functions.h"

namespace freedomstream {
class Decoder final : public webrtc::VideoDecoder {
 public:
  explicit Decoder(bool hevc) : hevc_(hevc) {}
  ~Decoder() override { Release(); }

  bool Configure(const Settings&) override {
    Stop();
    const char* pipeline = hevc_
      ? "appsrc name=input is-live=true format=time max-bytes=4194304 block=false caps=video/x-h265,stream-format=byte-stream,alignment=au ! h265parse ! d3d11h265dec ! d3d11download ! videoconvert ! video/x-raw,format=I420 ! appsink name=output sync=false max-buffers=2 drop=true"
      : "appsrc name=input is-live=true format=time max-bytes=4194304 block=false caps=video/x-h264,stream-format=byte-stream,alignment=au ! h264parse ! d3d11h264dec ! d3d11download ! videoconvert ! video/x-raw,format=I420 ! appsink name=output sync=false max-buffers=2 drop=true";
    GError* error = nullptr;
    pipeline_ = gst_parse_launch(pipeline, &error);
    if (error) { g_error_free(error); Stop(); return false; }
    if (!pipeline_) return false;
    input_ = gst_bin_get_by_name(GST_BIN(pipeline_), "input");
    output_ = gst_bin_get_by_name(GST_BIN(pipeline_), "output");
    if (!input_ || !output_) { Stop(); return false; }
    GstAppSinkCallbacks callbacks{};
    callbacks.new_sample = &Sample;
    gst_app_sink_set_callbacks(GST_APP_SINK(output_), &callbacks, this, nullptr);
    if (gst_element_set_state(pipeline_, GST_STATE_PLAYING) == GST_STATE_CHANGE_FAILURE) {
      Stop(); return false;
    }
    return true;
  }

  int32_t Decode(const webrtc::EncodedImage& image, bool, int64_t) override {
    if (!input_ || !image.data() || image.size() > 4194304)
      return WEBRTC_VIDEO_CODEC_ERROR;
    // Never accumulate compressed video while the device is stalled. A failed
    // decoder is reported to the watcher, which requests H.264 fallback.
    if (gst_app_src_get_current_level_bytes(GST_APP_SRC(input_)) > 4194304)
      return WEBRTC_VIDEO_CODEC_ERROR;
    GstBuffer* buffer = gst_buffer_new_allocate(nullptr, image.size(), nullptr);
    if (!buffer) return WEBRTC_VIDEO_CODEC_ERROR;
    gst_buffer_fill(buffer, 0, image.data(), image.size());
    const auto pts = next_pts_++ * GST_MSECOND;
    GST_BUFFER_PTS(buffer) = pts;
    GST_BUFFER_DTS(buffer) = pts;
    {
      std::lock_guard<std::mutex> lock(timestamps_mutex_);
      timestamps_[pts] = image.RtpTimestamp();
      while (timestamps_.size() > 128) timestamps_.erase(timestamps_.begin());
    }
    return gst_app_src_push_buffer(GST_APP_SRC(input_), buffer) == GST_FLOW_OK
      ? WEBRTC_VIDEO_CODEC_OK : WEBRTC_VIDEO_CODEC_ERROR;
  }

  int32_t RegisterDecodeCompleteCallback(webrtc::DecodedImageCallback* callback) override {
    callback_.store(callback);
    return WEBRTC_VIDEO_CODEC_OK;
  }
  int32_t Release() override {
    Stop(); // Joins GStreamer's callbacks before releasing WebRTC's callback.
    callback_.store(nullptr);
    return WEBRTC_VIDEO_CODEC_OK;
  }
  DecoderInfo GetDecoderInfo() const override {
    DecoderInfo info;
    info.implementation_name = "FreedomStream Direct3D11";
    info.is_hardware_accelerated = true;
    return info;
  }

 private:
  void Stop() {
    if (pipeline_) gst_element_set_state(pipeline_, GST_STATE_NULL);
    if (input_) gst_object_unref(input_);
    if (output_) gst_object_unref(output_);
    if (pipeline_) gst_object_unref(pipeline_);
    input_ = output_ = pipeline_ = nullptr;
    std::lock_guard<std::mutex> lock(timestamps_mutex_);
    timestamps_.clear();
  }
  static GstFlowReturn Sample(GstAppSink* sink, gpointer data) {
    return static_cast<Decoder*>(data)->Receive(sink);
  }
  GstFlowReturn Receive(GstAppSink* sink) {
    GstSample* sample = gst_app_sink_pull_sample(sink);
    if (!sample) return GST_FLOW_EOS;
    GstVideoInfo info;
    GstVideoFrame mapped;
    auto* buffer = gst_sample_get_buffer(sample);
    if (!gst_video_info_from_caps(&info, gst_sample_get_caps(sample)) ||
        info.width < 2 || info.height < 2 || info.width > 3840 || info.height > 3840 ||
        static_cast<int64_t>(info.width) * info.height > 3840LL * 2160 ||
        !gst_video_frame_map(&mapped, &info, buffer, GST_MAP_READ)) {
      gst_sample_unref(sample);
      return GST_FLOW_ERROR;
    }
    auto pixels = webrtc::I420Buffer::Create(info.width, info.height);
    libyuv::I420Copy(
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 0)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 0),
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 1)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 1),
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 2)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 2),
      pixels->MutableDataY(), pixels->StrideY(), pixels->MutableDataU(), pixels->StrideU(),
      pixels->MutableDataV(), pixels->StrideV(), info.width, info.height);
    gst_video_frame_unmap(&mapped);
    uint32_t timestamp = 0;
    bool found = false;
    {
      std::lock_guard<std::mutex> lock(timestamps_mutex_);
      const auto it = timestamps_.find(GST_BUFFER_PTS(buffer));
      if (it != timestamps_.end()) { timestamp = it->second; timestamps_.erase(it); found = true; }
    }
    gst_sample_unref(sample);
    if (found) {
      auto frame = webrtc::VideoFrame::Builder().set_video_frame_buffer(pixels)
        .set_timestamp_rtp(timestamp).build();
      if (auto* callback = callback_.load()) callback->Decoded(frame, std::nullopt, std::nullopt);
    }
    return GST_FLOW_OK;
  }
  const bool hevc_;
  GstElement *pipeline_ = nullptr, *input_ = nullptr, *output_ = nullptr;
  std::atomic<webrtc::DecodedImageCallback*> callback_{nullptr};
  std::mutex timestamps_mutex_;
  std::map<GstClockTime, uint32_t> timestamps_;
  uint64_t next_pts_ = 0;
};

class DecoderFactory final : public webrtc::VideoDecoderFactory {
 public:
  std::vector<webrtc::SdpVideoFormat> GetSupportedFormats() const override {
    std::vector<webrtc::SdpVideoFormat> formats;
    // d3d11 factories are registered for available adapters at GStreamer init.
    if (Available("d3d11h264dec")) formats = webrtc::SupportedH264DecoderCodecs();
    if (Available("d3d11h265dec")) formats.push_back(webrtc::SdpVideoFormat("H265"));
    return formats;
  }
  std::unique_ptr<webrtc::VideoDecoder> Create(
      const webrtc::Environment&, const webrtc::SdpVideoFormat& format) override {
    return std::make_unique<Decoder>(absl::EqualsIgnoreCase(format.name, "H265"));
  }
 private:
  static bool Available(const char* name) {
    if (!gst_is_initialized()) return false;
    GstElementFactory* factory = gst_element_factory_find(name);
    if (!factory) return false;
    gst_object_unref(factory);
    return true;
  }
};
} // namespace freedomstream
