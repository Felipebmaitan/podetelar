// Experimental bounded D3D11 staging. MIT licensed. Not linked by the app.
#pragma once
#include <limits>
#include <mutex>
#include <gst/d3d11/gstd3d11.h>
#include <wrl/client.h>
#include "api/video/i420_buffer.h"
#include "libyuv/convert.h"

namespace freedomstream {
class BoundedReadback {
 public:
  ~BoundedReadback() {
    staging_.Reset();
    if (device_) gst_object_unref(device_);
  }

  bool Copy(GstSample* sample, const GstVideoInfo& info, webrtc::I420Buffer* pixels) {
    std::lock_guard<std::mutex> guard(mutex_);
    auto* buffer = sample ? gst_sample_get_buffer(sample) : nullptr;
    if (!buffer || gst_buffer_n_memory(buffer) != 1 || !pixels) return false;
    auto* memory = gst_buffer_peek_memory(buffer, 0);
    if (!gst_is_d3d11_memory(memory)) return false;
    auto* dmem = GST_D3D11_MEMORY_CAST(memory);
    D3D11_TEXTURE2D_DESC desc{};
    if (!dmem->device || !gst_d3d11_memory_get_texture_desc(dmem, &desc) ||
        desc.Format != DXGI_FORMAT_NV12 || desc.MipLevels != 1 ||
        desc.SampleDesc.Count != 1 || !desc.ArraySize ||
        desc.Width < static_cast<UINT>(info.width) ||
        desc.Height < static_cast<UINT>(info.height) ||
        desc.Width > 4096 || desc.Height > 4096)
      return false;
    const UINT slice = gst_d3d11_memory_get_subresource_index(dmem);
    if (slice >= desc.ArraySize) return false;

    // GPU mapping observes GStreamer's transfer state without requesting its
    // per-memory CPU staging cache. The sample owns the memory and array slice.
    struct MemoryMap {
      GstMemory* memory;
      GstMapInfo map = GST_MAP_INFO_INIT;
      bool mapped;
      explicit MemoryMap(GstMemory* value) : memory(value), mapped(gst_memory_map(
          value, &map, static_cast<GstMapFlags>(GST_MAP_READ | GST_MAP_D3D11))) {}
      ~MemoryMap() { if (mapped) gst_memory_unmap(memory, &map); }
    } source_map(memory);
    if (!source_map.mapped) return false;
    auto* source = gst_d3d11_memory_get_resource_handle(dmem);
    if (!source) return false;
    auto* native_device = gst_d3d11_device_get_device_handle(dmem->device);
    auto* context = gst_d3d11_device_get_device_context_handle(dmem->device);
    if (!native_device || !context) return false;
    Microsoft::WRL::ComPtr<ID3D11Device> source_device;
    source->GetDevice(source_device.GetAddressOf());
    if (source_device.Get() != native_device) return false;

    struct DeviceLock {
      GstD3D11Device* device;
      explicit DeviceLock(GstD3D11Device* value) : device(value) { gst_d3d11_device_lock(device); }
      ~DeviceLock() { gst_d3d11_device_unlock(device); }
    } locked(dmem->device);
    if (device_ != dmem->device || width_ != desc.Width || height_ != desc.Height) {
      staging_.Reset();
      if (device_) gst_object_unref(device_);
      device_ = static_cast<GstD3D11Device*>(gst_object_ref(dmem->device));
      width_ = desc.Width;
      height_ = desc.Height;
    }
    if (!staging_) {
      D3D11_TEXTURE2D_DESC staging_desc{};
      staging_desc.Width = width_;
      staging_desc.Height = height_;
      staging_desc.MipLevels = staging_desc.ArraySize = 1;
      staging_desc.Format = DXGI_FORMAT_NV12;
      staging_desc.SampleDesc.Count = 1;
      staging_desc.Usage = D3D11_USAGE_STAGING;
      staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
      if (FAILED(native_device->CreateTexture2D(&staging_desc, nullptr, staging_.GetAddressOf())))
        return false;
    }
    context->CopySubresourceRegion(staging_.Get(), 0, 0, 0, 0, source, slice, nullptr);
    D3D11_MAPPED_SUBRESOURCE mapped{};
    if (FAILED(context->Map(staging_.Get(), 0, D3D11_MAP_READ, 0, &mapped))) return false;
    bool converted = false;
    if (mapped.pData && mapped.RowPitch >= width_ &&
        mapped.RowPitch <= static_cast<UINT>(std::numeric_limits<int>::max())) {
      const auto* y = static_cast<const uint8_t*>(mapped.pData);
      // NV12 chroma follows the allocated (possibly padded) texture height,
      // rather than the visible video height. Copy only visible pixels.
      const auto* uv = y + static_cast<size_t>(mapped.RowPitch) * height_;
      converted = libyuv::NV12ToI420(y, static_cast<int>(mapped.RowPitch), uv,
          static_cast<int>(mapped.RowPitch), pixels->MutableDataY(), pixels->StrideY(),
          pixels->MutableDataU(), pixels->StrideU(), pixels->MutableDataV(), pixels->StrideV(),
          info.width, info.height) == 0;
    }
    context->Unmap(staging_.Get(), 0);
    return converted;
  }

 private:
  std::mutex mutex_;
  GstD3D11Device* device_ = nullptr;
  Microsoft::WRL::ComPtr<ID3D11Texture2D> staging_;
  UINT width_ = 0, height_ = 0;
};
}  // namespace freedomstream
