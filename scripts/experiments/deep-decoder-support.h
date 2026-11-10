// MIT. Diagnostic/candidate code copied only into the isolated experimental SDK.
#pragma once
#include <atomic>
#include <chrono>
#include <cstdlib>
#include <map>
#include <memory>
#include <mutex>
#include <set>
#include <thread>
#include <string>
#include <iterator>
#include <cstring>
#include <gst/d3d11/gstd3d11.h>
namespace deep {
using Clock=std::chrono::steady_clock;
inline uint64_t Ns(Clock::duration d){return std::chrono::duration_cast<std::chrono::nanoseconds>(d).count();}
inline bool Flag(const char* key){const auto* v=std::getenv(key);return v&&std::string(v)=="1";}
inline bool Shared(){static const bool b=Flag("FREEDOMSTREAM_DEEP_SHARED_DEVICE");return b;}
inline bool Relaxed(){static const bool b=Flag("FREEDOMSTREAM_DEEP_RELAXED_MAP");return b;}
inline bool FlushedPolling(){static const bool b=Flag("FREEDOMSTREAM_DEEP_FLUSH_MAP");return b;}
inline std::atomic<uint64_t> devices_created{0},devices_live{0},samples_live{0},samples_peak{0},deferred_live{0},planes_live_bytes{0},staging_live{0};
inline std::atomic<uint64_t> readbacks{0},readback_lock_ns{0},device_lock_ns{0},copy_ns{0},map_ns{0},convert_ns{0},map_calls{0},map_busy{0},map_fallbacks{0};
inline std::atomic<uint64_t> materialized{0},failures{0},array_samples{0},single_samples{0},adapters_mismatch{0};
inline std::atomic<uint64_t> bounded_success{0},generic_fallback{0},map_errors{0},direct_logs{0},copy_logs{0},pool_full_logs{0};
inline std::atomic<uint64_t> trace_messages{0};
inline void Log(GstDebugCategory*,GstDebugLevel,const gchar*,const gchar*,gint,GObject*,GstDebugMessage* message,gpointer){trace_messages++;const auto* text=gst_debug_message_get(message);if(std::strstr(text,"Can do direct rendering"))direct_logs++;if(std::strstr(text,"Copying to output D3D11 texture"))copy_logs++;if(std::strstr(text,"memory pool is about to full"))pool_full_logs++;}
inline void InstallTrace(){static std::once_flag once;std::call_once(once,[]{if(Flag("FREEDOMSTREAM_DEEP_POOL_TRACE")){gst_debug_remove_log_function(gst_debug_log_default);gst_debug_add_log_function(Log,nullptr,nullptr);gst_debug_set_active(TRUE);gst_debug_set_threshold_for_name("d3d11decoder",GST_LEVEL_TRACE);}});}
inline std::mutex devices_mutex;
inline std::set<GstD3D11Device*> devices;
inline std::set<gint64> adapters;
inline std::mutex samples_mutex;
inline std::set<GstSample*> samples;
inline void DeviceGone(gpointer data,GObject*){std::lock_guard<std::mutex> lock(devices_mutex);devices.erase(static_cast<GstD3D11Device*>(data));devices_live--;}
inline void SampleGone(gpointer data,GstMiniObject*){std::lock_guard<std::mutex> lock(samples_mutex);samples.erase(static_cast<GstSample*>(data));samples_live--;}
inline void Observe(GstSample* sample){
 {std::lock_guard<std::mutex> lock(samples_mutex);if(samples.insert(sample).second){auto live=++samples_live;auto peak=samples_peak.load();while(live>peak&&!samples_peak.compare_exchange_weak(peak,live)){}gst_mini_object_weak_ref(GST_MINI_OBJECT_CAST(sample),SampleGone,sample);}}
 auto* b=gst_sample_get_buffer(sample);if(!b||gst_buffer_n_memory(b)!=1)return;
 auto* m=gst_buffer_peek_memory(b,0);if(!gst_is_d3d11_memory(m))return;
 auto* d=GST_D3D11_MEMORY_CAST(m);D3D11_TEXTURE2D_DESC desc{};
 if(gst_d3d11_memory_get_texture_desc(d,&desc)){if(desc.ArraySize>1)array_samples++;else single_samples++;}
 std::lock_guard<std::mutex> lock(devices_mutex);
 if(devices.insert(d->device).second){gint64 luid=0;g_object_get(d->device,"adapter-luid",&luid,nullptr);adapters.insert(luid);devices_created++;devices_live++;g_object_weak_ref(G_OBJECT(d->device),DeviceGone,d->device);}
}
struct Device {GstD3D11Device* value;explicit Device(GstD3D11Device* p):value(p){}~Device(){gst_object_unref(value);}};
inline std::mutex shared_mutex;
inline std::map<gint64,std::weak_ptr<Device>> active_devices;
inline std::shared_ptr<Device> Setup(GstElement* pipeline){
 if(!Shared())return {};
 auto* decoder=gst_bin_get_by_name(GST_BIN(pipeline),"fsdeepdecoder");if(!decoder)return {};
 gint64 luid=0;g_object_get(decoder,"adapter-luid",&luid,nullptr);
 std::shared_ptr<Device> result;
 {std::lock_guard<std::mutex> lock(shared_mutex);result=active_devices[luid].lock();
  if(!result){auto* device=gst_d3d11_device_new_for_adapter_luid(luid,D3D11_CREATE_DEVICE_BGRA_SUPPORT);if(device){result=std::make_shared<Device>(device);active_devices[luid]=result;}}}
 if(result){auto* context=gst_d3d11_context_new(result->value);gst_element_set_context(pipeline,context);gst_element_set_context(decoder,context);gst_context_unref(context);}
 gst_object_unref(decoder);return result;
}
// Called with the GStreamer immediate-context lock held. Every return leaves it held.
inline HRESULT Map(ID3D11DeviceContext* context,ID3D11Resource* resource,GstD3D11Device* device,D3D11_MAPPED_SUBRESOURCE* out){
 const auto start=Clock::now();HRESULT hr;
 if(!Relaxed()){map_calls++;hr=context->Map(resource,0,D3D11_MAP_READ,0,out);}
 else {
  if(FlushedPolling())context->Flush();
  const auto deadline=start+std::chrono::milliseconds(8);
  for(;;){map_calls++;hr=context->Map(resource,0,D3D11_MAP_READ,D3D11_MAP_FLAG_DO_NOT_WAIT,out);
   if(hr!=DXGI_ERROR_WAS_STILL_DRAWING)break;
   map_busy++;
   if(Clock::now()>=deadline){map_fallbacks++;map_calls++;hr=context->Map(resource,0,D3D11_MAP_READ,0,out);break;}
   gst_d3d11_device_unlock(device);std::this_thread::sleep_for(std::chrono::milliseconds(1));
   const auto acquire=Clock::now();gst_d3d11_device_lock(device);device_lock_ns+=Ns(Clock::now()-acquire);
  }
 }
 if(FAILED(hr))map_errors++;
 map_ns+=Ns(Clock::now()-start);return hr;
}
// Fixed numeric schema: never exports pointers, LUIDs, media, names or authority.
inline void Snapshot(uint64_t* out,size_t count){
 const uint64_t values[]={devices_created.load(),devices_live.load(),samples_live.load(),samples_peak.load(),deferred_live.load(),planes_live_bytes.load(),staging_live.load(),readbacks.load(),readback_lock_ns.load(),device_lock_ns.load(),copy_ns.load(),map_ns.load(),convert_ns.load(),map_calls.load(),map_busy.load(),map_fallbacks.load(),materialized.load(),failures.load(),array_samples.load(),single_samples.load(),bounded_success.load(),generic_fallback.load(),map_errors.load(),direct_logs.load(),copy_logs.load(),pool_full_logs.load(),trace_messages.load()};
 for(size_t i=0;i<count&&i<std::size(values);i++)out[i]=values[i];
}
}
extern "C" __declspec(dllexport) void freedomstream_deep_metrics(uint64_t* values,size_t count){deep::Snapshot(values,count);}
