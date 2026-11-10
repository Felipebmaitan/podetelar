// Copyright (c) 2026 FreedomStream contributors. MIT licensed.
// Retain decoded GPU storage until a consumer reads the supported I420 planes.
#pragma once

#include <atomic>
#include <cstdint>
#include <cstring>
#include <memory>
#include <mutex>
#include <utility>
#include <gst/app/gstappsink.h>
#include <gst/video/video.h>
#include "api/make_ref_counted.h"
#include "api/video/i420_buffer.h"
#include "libyuv/convert.h"
#include "bounded_readback.h"

namespace freedomstream {
#ifdef FREEDOMSTREAM_DECODER_PROBE
inline std::atomic<uint64_t> materializations{0}, failed_materializations{0};
#endif


class DeferredI420Buffer : public webrtc::I420BufferInterface {
 public:
  static webrtc::scoped_refptr<DeferredI420Buffer> Create(
      GstSample* sample, const GstVideoInfo& info,
      std::shared_ptr<std::atomic<bool>> failed, std::shared_ptr<BoundedReadback> readback = nullptr) {
    if (!sample || !failed || GST_VIDEO_INFO_FORMAT(&info) != GST_VIDEO_FORMAT_NV12 ||
        info.width < 2 || info.height < 2 || info.width > 3840 || info.height > 3840 ||
        static_cast<int64_t>(info.width) * info.height > 3840LL * 2160)
      return nullptr;
    auto* buffer = gst_sample_get_buffer(sample);
    if (!buffer) return nullptr;
    const auto* crop = gst_buffer_get_video_crop_meta(buffer);
    if (crop && (crop->x || crop->y || crop->width != static_cast<guint>(info.width) ||
                 crop->height != static_cast<guint>(info.height)))
      return nullptr;
    return webrtc::make_ref_counted<DeferredI420Buffer>(sample, info, std::move(failed), std::move(readback));
  }

  DeferredI420Buffer(GstSample* sample, const GstVideoInfo& info,
                     std::shared_ptr<std::atomic<bool>> failed, std::shared_ptr<BoundedReadback> readback = nullptr)
      : sample_(gst_sample_ref(sample)), info_(info), failed_(std::move(failed)), readback_(std::move(readback)) {}

  int width() const override { return info_.width; }
  int height() const override { return info_.height; }
  int StrideY() const override { return info_.width; }
  int StrideU() const override { return (info_.width + 1) / 2; }
  int StrideV() const override { return (info_.width + 1) / 2; }
  const uint8_t* DataY() const override { return Pixels()->DataY(); }
  const uint8_t* DataU() const override { return Pixels()->DataU(); }
  const uint8_t* DataV() const override { return Pixels()->DataV(); }

 protected:
  ~DeferredI420Buffer() override {
    if (sample_) gst_sample_unref(sample_);
  }

 private:
  const webrtc::I420Buffer* Pixels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!pixels_) {
#ifdef FREEDOMSTREAM_DECODER_PROBE
      materializations.fetch_add(1, std::memory_order_relaxed);
#endif
      auto pixels = webrtc::I420Buffer::Create(width(), height(), StrideY(), StrideU(), StrideV());
      GstVideoFrame mapped{};
      bool converted = false;
      auto* buffer = sample_ ? gst_sample_get_buffer(sample_) : nullptr;
      // Mapping a D3D11 buffer through the generic video-frame API performs
      // synchronized staging/readback. Retaining the GstSample also retains its
      // buffer, array slice and device; a naked texture pointer is insufficient.
      if (readback_) { converted = readback_->Copy(sample_, info_, pixels.get()); }
      else if (buffer && gst_video_frame_map(&mapped, &info_, buffer, GST_MAP_READ)) {
        converted = libyuv::NV12ToI420(
            static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 0)),
            GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 0),
            static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 1)),
            GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 1),
            pixels->MutableDataY(), StrideY(), pixels->MutableDataU(), StrideU(),
            pixels->MutableDataV(), StrideV(), width(), height()) == 0;
        gst_video_frame_unmap(&mapped);
      }
      if (!converted) {
        // Never expose uninitialized memory on device/read failure. Signal the
        // established codec error on the next input. Each Configure owns a new
        // flag, so a late old frame cannot poison its replacement decoder.
        std::memset(pixels->MutableDataY(), 16, static_cast<size_t>(StrideY()) * height());
        std::memset(pixels->MutableDataU(), 128,
                    static_cast<size_t>(StrideU()) * ((height() + 1) / 2));
        std::memset(pixels->MutableDataV(), 128,
                    static_cast<size_t>(StrideV()) * ((height() + 1) / 2));
        failed_->store(true, std::memory_order_release);
#ifdef FREEDOMSTREAM_DECODER_PROBE
        failed_materializations.fetch_add(1, std::memory_order_relaxed);
#endif
      }
      pixels_ = std::move(pixels);
      readback_.reset();
      gst_sample_unref(sample_);
      sample_ = nullptr;
    }
    // The resulting planes are immutable and live until this buffer's last
    // owner is released. Other readers serialize only the first conversion.
    return pixels_.get();
  }

  mutable GstSample* sample_;
  const GstVideoInfo info_;
  const std::shared_ptr<std::atomic<bool>> failed_;
  mutable std::shared_ptr<BoundedReadback> readback_;
  mutable std::mutex mutex_;
  mutable webrtc::scoped_refptr<webrtc::I420Buffer> pixels_;
};
} // namespace freedomstream
