// Copyright (c) 2026 PodeTelar contributors. MIT licensed.
// Native WebRTC decoder adapter for the separately licensed bundled GStreamer.
#pragma once

#include <atomic>
#include <cstdlib>
#include <vector>
#include "libyuv/convert.h"
#include <map>
#include <mutex>
#ifndef GST_USE_UNSTABLE_API
#define GST_USE_UNSTABLE_API
#endif
#include <gst/codecparsers/gstav1parser.h>
#include <gst/app/gstappsrc.h>
#include <gst/app/gstappsink.h>
#include <gst/video/video.h>
#include <VideoToolbox/VideoToolbox.h>
#include "api/video/i420_buffer.h"
#include "api/video/video_frame.h"
#include "modules/video_coding/include/video_error_codes.h"

namespace podetelar {
class Decoder final : public webrtc::VideoDecoder {
 public:
  explicit Decoder(bool hevc, bool av1 = false) : hevc_(hevc), av1_(av1) {}
  ~Decoder() override { Release(); }

  bool Configure(const Settings&) override {
    Stop();
    failed_generation_ = std::make_shared<std::atomic<bool>>(false);
    // vtdec_hw requires a hardware VideoToolbox session; no software fallback.
    const char* pipeline = av1_
      ? "appsrc name=input is-live=true format=time max-bytes=4194304 block=false caps=video/x-av1,stream-format=obu-stream,alignment=tu ! av1parse ! video/x-av1,stream-format=obu-stream,alignment=tu ! appsink name=output sync=false max-buffers=2 drop=true enable-last-sample=false"
      : hevc_
      ? "appsrc name=input is-live=true format=time max-bytes=4194304 block=false caps=video/x-h265,stream-format=byte-stream,alignment=au ! h265parse ! video/x-h265,stream-format=hvc1,alignment=au ! appsink name=output sync=false max-buffers=2 drop=true enable-last-sample=false"
      : "appsrc name=input is-live=true format=time max-bytes=4194304 block=false caps=video/x-h264,stream-format=byte-stream,alignment=au ! h264parse ! vtdec_hw ! videoconvert ! video/x-raw,format=I420 ! appsink name=output sync=false max-buffers=2 drop=true enable-last-sample=false";
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
    if (failed_generation_->load(std::memory_order_acquire)) return WEBRTC_VIDEO_CODEC_ERROR;
    if (!input_ || !image.data() || image.size() > 4194304 ||
        gst_app_src_get_current_level_bytes(GST_APP_SRC(input_)) > 4194304)
      return WEBRTC_VIDEO_CODEC_ERROR;
    GstBus* bus = gst_element_get_bus(pipeline_);
    GstMessage* error = gst_bus_pop_filtered(bus, GST_MESSAGE_ERROR);
    gst_object_unref(bus);
    if (error) { gst_message_unref(error); return WEBRTC_VIDEO_CODEC_ERROR; }
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
    info.implementation_name = "PodeTelar VideoToolbox";
    info.is_hardware_accelerated = true;
    return info;
  }

 private:
  void Stop() {
    if (pipeline_) gst_element_set_state(pipeline_, GST_STATE_NULL);
    StopNative();
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
    if (hevc_ || av1_) { const auto result = ReceiveNative(sample); gst_sample_unref(sample); return result; }
    GstVideoInfo info;
    auto* buffer = gst_sample_get_buffer(sample);
    if (!gst_video_info_from_caps(&info, gst_sample_get_caps(sample)) ||
        info.width < 2 || info.height < 2 || info.width > 3840 || info.height > 3840 ||
        static_cast<int64_t>(info.width) * info.height > 3840LL * 2160 ||
        GST_VIDEO_INFO_FORMAT(&info) != GST_VIDEO_FORMAT_I420) {
      gst_sample_unref(sample);
      return GST_FLOW_ERROR;
    }
    GstVideoFrame mapped{};
    if (!gst_video_frame_map(&mapped, &info, buffer, GST_MAP_READ)) {
      failed_generation_->store(true, std::memory_order_release);
      gst_sample_unref(sample); return GST_FLOW_ERROR;
    }
    auto pixels = webrtc::I420Buffer::Copy(info.width, info.height,
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 0)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 0),
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 1)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 1),
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 2)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 2));
    gst_video_frame_unmap(&mapped);
    if (!pixels) { gst_sample_unref(sample); return GST_FLOW_ERROR; }
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
      if (auto* callback = callback_.load()) {
        callback->Decoded(frame, std::nullopt, std::nullopt);
      }
    }
    return GST_FLOW_OK;
  }

  // GStreamer 1.28.8 holds sixteen HEVC output pictures regardless of the SPS
  // reordering bound. WebRTC retains ten frame associations, so that queue loses
  // every decoded frame. Parse/framing stays in GStreamer; HEVC and AV1 use the
  // public VideoToolbox API directly with one bounded in-flight sample.
  template<class T> struct Cf {
    T value = nullptr;
    explicit Cf(T v = nullptr) : value(v) {}
    ~Cf() { if (value) CFRelease(value); }
    Cf(const Cf&) = delete;
    Cf& operator=(const Cf&) = delete;
  };
  void StopNative() {
    if (native_session_) {
      VTDecompressionSessionWaitForAsynchronousFrames(native_session_);
      VTDecompressionSessionInvalidate(native_session_);
      CFRelease(native_session_); native_session_ = nullptr;
    }
    if (native_format_) { CFRelease(native_format_); native_format_ = nullptr; }
    native_config_.clear();
  }
  // av1parse does not supply av1C for an OBU stream. Build the configuration
  // record from the parsed sequence header instead of passing a null codec_data
  // to GStreamer 1.28.8's AV1 decoder. See AOM AV1-ISOBMFF section 2.3.3.
  bool Av1Config(GstBuffer* buffer, std::vector<uint8_t>& config) {
    GstMapInfo map{};
    if (!gst_buffer_map(buffer,&map,GST_MAP_READ)) return false;
    auto* parser = gst_av1_parser_new();
    bool valid = parser != nullptr;
    size_t offset = 0;
    while (valid && offset < map.size) {
      GstAV1OBU obu{};
      guint32 consumed = 0;
      if (gst_av1_parser_identify_one_obu(parser,map.data+offset,map.size-offset,&obu,&consumed) != GST_AV1_PARSER_OK || !consumed || consumed > map.size-offset) { valid = false; break; }
      if (obu.obu_type == GST_AV1_OBU_SEQUENCE_HEADER) {
        GstAV1SequenceHeaderOBU sequence{};
        if (consumed > 65532 || gst_av1_parser_parse_sequence_header_obu(parser,&obu,&sequence) != GST_AV1_PARSER_OK || sequence.seq_profile != 0 || sequence.bit_depth != 8) { valid = false; break; }
        const auto& color = sequence.color_config;
        const auto& point = sequence.operating_points[0];
        config = {0x81, static_cast<uint8_t>((sequence.seq_profile << 5) | point.seq_level_idx),
          static_cast<uint8_t>((point.seq_tier << 7) | (color.high_bitdepth << 6) | (color.twelve_bit << 5) | (color.mono_chrome << 4) | (color.subsampling_x << 3) | (color.subsampling_y << 2) | color.chroma_sample_position), 0};
        config.insert(config.end(),map.data+offset,map.data+offset+consumed);
        break;
      }
      offset += consumed;
    }
    if (parser) gst_av1_parser_free(parser);
    gst_buffer_unmap(buffer,&map);
    return valid;
  }
  bool ConfigureNative(const GstStructure* caps, GstBuffer* buffer) {
    int width = 0, height = 0;
    if (!gst_structure_get_int(caps,"width",&width) || !gst_structure_get_int(caps,"height",&height)
        || width < 2 || height < 2 || width > 3840 || height > 3840
        || static_cast<int64_t>(width)*height > 3840LL*2160) return false;
    std::vector<uint8_t> config;
    if (av1_) {
      if (!Av1Config(buffer,config)) return false;
      if (config.empty()) return native_session_ != nullptr;
    } else {
      const auto* value = gst_structure_get_value(caps,"codec_data");
      GstBuffer* codec = value && GST_VALUE_HOLDS_BUFFER(value) ? gst_value_get_buffer(value) : nullptr;
      GstMapInfo map{};
      if (!codec || !gst_buffer_map(codec,&map,GST_MAP_READ)) return false;
      if (!map.size || map.size > 65536) { gst_buffer_unmap(codec,&map); return false; }
      config.assign(map.data,map.data+map.size);
      gst_buffer_unmap(codec,&map);
    }
    if (native_session_ && config == native_config_) return true;
    StopNative();
    Cf<CFDataRef> bytes(CFDataCreate(kCFAllocatorDefault,config.data(),config.size()));
    if (!bytes.value) return false;
    const void* atom_key = av1_ ? CFSTR("av1C") : CFSTR("hvcC"); const void* atom_value = bytes.value;
    Cf<CFDictionaryRef> atoms(CFDictionaryCreate(kCFAllocatorDefault,&atom_key,&atom_value,1,&kCFTypeDictionaryKeyCallBacks,&kCFTypeDictionaryValueCallBacks));
    if (!atoms.value) return false;
    const void* extension_key = kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms; const void* extension_value = atoms.value;
    Cf<CFDictionaryRef> extensions(CFDictionaryCreate(kCFAllocatorDefault,&extension_key,&extension_value,1,&kCFTypeDictionaryKeyCallBacks,&kCFTypeDictionaryValueCallBacks));
    if (!extensions.value) return false;
    if (CMVideoFormatDescriptionCreate(kCFAllocatorDefault,av1_ ? kCMVideoCodecType_AV1 : kCMVideoCodecType_HEVC,width,height,extensions.value,&native_format_) != noErr) return false;
    const void* required_key = kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder;
    const void* required_value = kCFBooleanTrue;
    Cf<CFDictionaryRef> specification(CFDictionaryCreate(kCFAllocatorDefault,&required_key,&required_value,1,&kCFTypeDictionaryKeyCallBacks,&kCFTypeDictionaryValueCallBacks));
    if (!specification.value) { StopNative(); return false; }
    int32_t pixel_type = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
    Cf<CFNumberRef> pixel(CFNumberCreate(kCFAllocatorDefault,kCFNumberSInt32Type,&pixel_type));
    if (!pixel.value) { StopNative(); return false; }
    const void* pixel_key = kCVPixelBufferPixelFormatTypeKey; const void* pixel_value = pixel.value;
    Cf<CFDictionaryRef> attributes(CFDictionaryCreate(kCFAllocatorDefault,&pixel_key,&pixel_value,1,&kCFTypeDictionaryKeyCallBacks,&kCFTypeDictionaryValueCallBacks));
    if (!attributes.value) { StopNative(); return false; }
    VTDecompressionOutputCallbackRecord callback{&NativeOutput,this};
    if (VTDecompressionSessionCreate(kCFAllocatorDefault,native_format_,specification.value,attributes.value,&callback,&native_session_) != noErr) { StopNative(); return false; }
    Cf<CFTypeRef> hardware;
    if (VTSessionCopyProperty(native_session_,kVTDecompressionPropertyKey_UsingHardwareAcceleratedVideoDecoder,kCFAllocatorDefault,&hardware.value) != noErr || hardware.value != kCFBooleanTrue) { StopNative(); return false; }
    VTSessionSetProperty(native_session_,kVTDecompressionPropertyKey_RealTime,kCFBooleanTrue);
    native_config_ = std::move(config);
    return true;
  }
  GstFlowReturn ReceiveNative(GstSample* sample) {
    if (!ConfigureNative(gst_caps_get_structure(gst_sample_get_caps(sample),0),gst_sample_get_buffer(sample))) {
      failed_generation_->store(true,std::memory_order_release); return GST_FLOW_ERROR;
    }
    auto* buffer = gst_sample_get_buffer(sample);
    uint32_t timestamp;
    {
      std::lock_guard<std::mutex> lock(timestamps_mutex_);
      const auto it = timestamps_.find(GST_BUFFER_PTS(buffer));
      if (it == timestamps_.end()) return GST_FLOW_OK;
      timestamp = it->second; timestamps_.erase(it);
    }
    GstMapInfo map{};
    if (!gst_buffer_map(buffer,&map,GST_MAP_READ)) return GST_FLOW_ERROR;
    // The GstSample owns these bytes until the synchronous wait below completes.
    Cf<CMBlockBufferRef> block;
    Cf<CMSampleBufferRef> compressed;
    size_t size = map.size;
    OSStatus status = CMBlockBufferCreateWithMemoryBlock(kCFAllocatorDefault,map.data,size,kCFAllocatorNull,nullptr,0,size,0,&block.value);
    if (status == noErr) status = CMSampleBufferCreateReady(kCFAllocatorDefault,block.value,native_format_,1,0,nullptr,1,&size,&compressed.value);
    if (status == noErr) status = VTDecompressionSessionDecodeFrame(native_session_,compressed.value,kVTDecodeFrame_EnableAsynchronousDecompression,reinterpret_cast<void*>(static_cast<uintptr_t>(timestamp)),nullptr);
    if (status == noErr) status = VTDecompressionSessionWaitForAsynchronousFrames(native_session_);
    gst_buffer_unmap(buffer,&map);
    if (status != noErr) failed_generation_->store(true,std::memory_order_release);
    return status == noErr ? GST_FLOW_OK : GST_FLOW_ERROR;
  }
  static void NativeOutput(void* context, void* reference, OSStatus status, VTDecodeInfoFlags, CVImageBufferRef image, CMTime, CMTime) {
    auto* self = static_cast<Decoder*>(context);
    if (status != noErr) { self->failed_generation_->store(true,std::memory_order_release); return; }
    if (!image) return;
    const auto width = CVPixelBufferGetWidth(image), height = CVPixelBufferGetHeight(image);
    if (width < 2 || height < 2 || width > 3840 || height > 3840 || width*height > 3840ULL*2160
        || CVPixelBufferGetPixelFormatType(image) != kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
        || CVPixelBufferGetPlaneCount(image) != 2 || CVPixelBufferLockBaseAddress(image,kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) {
      self->failed_generation_->store(true,std::memory_order_release); return;
    }
    auto pixels = webrtc::I420Buffer::Create(width,height);
    const auto* y = static_cast<const uint8_t*>(CVPixelBufferGetBaseAddressOfPlane(image,0));
    const auto* uv = static_cast<const uint8_t*>(CVPixelBufferGetBaseAddressOfPlane(image,1));
    const bool copied = y && uv && libyuv::NV12ToI420(y,CVPixelBufferGetBytesPerRowOfPlane(image,0),uv,CVPixelBufferGetBytesPerRowOfPlane(image,1),pixels->MutableDataY(),pixels->StrideY(),pixels->MutableDataU(),pixels->StrideU(),pixels->MutableDataV(),pixels->StrideV(),width,height) == 0;
    CVPixelBufferUnlockBaseAddress(image,kCVPixelBufferLock_ReadOnly);
    if (!copied) { self->failed_generation_->store(true,std::memory_order_release); return; }
    auto frame = webrtc::VideoFrame::Builder().set_video_frame_buffer(pixels).set_timestamp_rtp(static_cast<uint32_t>(reinterpret_cast<uintptr_t>(reference))).build();
    if (auto* callback = self->callback_.load()) callback->Decoded(frame,std::nullopt,std::nullopt);
  }
  VTDecompressionSessionRef native_session_ = nullptr;
  CMVideoFormatDescriptionRef native_format_ = nullptr;
  std::vector<uint8_t> native_config_;
  std::shared_ptr<std::atomic<bool>> failed_generation_ = std::make_shared<std::atomic<bool>>(false);
  const bool hevc_;
  const bool av1_;
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
    if (Available("vtdec_hw")) {
      if (VTIsHardwareDecodeSupported(kCMVideoCodecType_H264)) formats = webrtc::SupportedH264DecoderCodecs();
      if (VTIsHardwareDecodeSupported(kCMVideoCodecType_HEVC)) formats.push_back(webrtc::SdpVideoFormat("H265"));
      VTRegisterSupplementalVideoDecoderIfAvailable(kCMVideoCodecType_AV1);
      if (Available("av1parse") && VTIsHardwareDecodeSupported(kCMVideoCodecType_AV1))
        formats.push_back(webrtc::SdpVideoFormat::AV1Profile0());
    }
    return formats;
  }
  std::unique_ptr<webrtc::VideoDecoder> Create(
      const webrtc::Environment&, const webrtc::SdpVideoFormat& format) override {
    if (!format.IsCodecInList(GetSupportedFormats())) return nullptr;
    return std::make_unique<Decoder>(absl::EqualsIgnoreCase(format.name, "H265"), absl::EqualsIgnoreCase(format.name, "AV1"));
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
} // namespace podetelar
