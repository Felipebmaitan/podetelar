// Copyright (c) 2026 FreedomStream contributors. MIT licensed.
// Independent local probe linked to the pinned release WebRTC, never shipped.
#define NOMINMAX
#include <windows.h>
#include <shellapi.h>
#include <gst/app/gstappsink.h>
#include <gst/app/gstappsrc.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <fstream>
#include <mutex>
#include <thread>
#include <vector>
#include "api/video_codecs/video_decoder.h"
#include "modules/video_coding/codecs/av1/dav1d_decoder.h"
#include "decoder-probe-adapter.h"
using Clock=std::chrono::steady_clock;
double Ms(Clock::duration time){return std::chrono::duration<double,std::milli>(time).count();}
double Cpu(){FILETIME c{},e{},k{},u{};GetProcessTimes(GetCurrentProcess(),&c,&e,&k,&u);auto ticks=[](FILETIME t){return (uint64_t(t.dwHighDateTime)<<32)|t.dwLowDateTime;};return double(ticks(k)+ticks(u))/10000.0;}
struct Packet { std::vector<uint8_t> bytes; bool key; };
std::vector<Packet> ReadFixture(const wchar_t* filename,bool av1){
 auto* name=g_utf16_to_utf8(reinterpret_cast<const gunichar2*>(filename),-1,nullptr,nullptr,nullptr);
 std::string description=av1?"filesrc name=source ! video/x-av1,framerate=60/1 ! av1parse ! video/x-av1,stream-format=obu-stream,alignment=tu ! appsink name=out sync=false max-buffers=4 drop=false":"filesrc name=source ! h265parse config-interval=-1 ! video/x-h265,stream-format=byte-stream,alignment=au ! appsink name=out sync=false max-buffers=4 drop=false";
 auto* pipeline=gst_parse_launch(description.c_str(),nullptr);if(!pipeline){g_free(name);return {};}
 auto* source=gst_bin_get_by_name(GST_BIN(pipeline),"source");g_object_set(source,"location",name,nullptr);g_free(name);gst_object_unref(source);
 auto* sink=gst_bin_get_by_name(GST_BIN(pipeline),"out");gst_element_set_state(pipeline,GST_STATE_PLAYING);
 std::vector<Packet> result;auto start=Clock::now();size_t total=0;
 while(Ms(Clock::now()-start)<10000&&result.size()<601){auto* sample=gst_app_sink_try_pull_sample(GST_APP_SINK(sink),GST_MSECOND*50);if(!sample){if(gst_app_sink_is_eos(GST_APP_SINK(sink)))break;continue;}auto* buffer=gst_sample_get_buffer(sample);GstMapInfo mapped{};if(gst_buffer_map(buffer,&mapped,GST_MAP_READ)){total+=mapped.size;if(total<64*1024*1024)result.push_back({std::vector<uint8_t>(mapped.data,mapped.data+mapped.size),!GST_BUFFER_FLAG_IS_SET(buffer,GST_BUFFER_FLAG_DELTA_UNIT)});gst_buffer_unmap(buffer,&mapped);}gst_sample_unref(sample);}
 gst_element_set_state(pipeline,GST_STATE_NULL);gst_object_unref(sink);gst_object_unref(pipeline);return result;
}
struct Callback final:webrtc::DecodedImageCallback {
 std::atomic<int> delivered{0};std::atomic<bool> stalled{false};bool stall=false;Clock::time_point start;std::mutex mutex;std::vector<double> ages;
 int32_t Decoded(webrtc::VideoFrame& frame) override {
  const int count=++delivered;if(stall&&count==30&&!stalled.exchange(true))std::this_thread::sleep_for(std::chrono::milliseconds(750));
  const double age=Ms(Clock::now()-start)-double(frame.rtp_timestamp())/90.0;
  {std::lock_guard<std::mutex> lock(mutex);ages.push_back(age);}return 0;
 }
};
int WINAPI wWinMain(HINSTANCE,HINSTANCE,PWSTR,int){
 int argc=0;auto** args=CommandLineToArgvW(GetCommandLineW(),&argc);if(argc<4)return 2;
 const std::wstring mode=args[1];const bool av1=mode==L"av1";const bool bounded=mode==L"bounded";
 const int contexts=argc>4?_wtoi(args[4]):1;const int cores=argc>5?_wtoi(args[5]):4;
 if(contexts<1||contexts>4||cores<1||cores>16)return 2;
 gst_init(nullptr,nullptr);const auto packets=ReadFixture(args[2],av1);std::ofstream report(args[3]);if(packets.size()!=300){report<<"{\"completed\":false,\"stage\":\"fixture\",\"packets\":"<<packets.size()<<"}";return 3;}
 std::atomic<int> delivered{0},restarts{0},skipped{0},max_pending{0},errors{0};std::mutex timing_mutex;std::vector<double> ages;
 const auto start=Clock::now();const double cpu=Cpu();std::vector<std::thread> workers;
 for(int worker=0;worker<contexts;worker++)workers.emplace_back([&,worker]{
  std::unique_ptr<webrtc::VideoDecoder> decoder=av1?webrtc::CreateDav1dDecoder():std::unique_ptr<webrtc::VideoDecoder>(new freedomstream::Decoder(true));
  Callback callback;callback.start=start;callback.stall=!av1;webrtc::VideoDecoder::Settings settings;settings.set_number_of_cores(cores);settings.set_codec_type(av1?webrtc::kVideoCodecAV1:webrtc::kVideoCodecH265);
  if(!decoder||!decoder->Configure(settings)){errors++;return;}decoder->RegisterDecodeCompleteCallback(&callback);
  int submitted=0,base_delivered=0;bool need_key=false;
  for(size_t i=0;i<packets.size();i++){
   const auto due=start+std::chrono::microseconds(i*1000000/60);std::this_thread::sleep_until(due);
   const auto& packet=packets[i];
   if(need_key){if(!packet.key||Ms(Clock::now()-due)>50){skipped++;continue;}need_key=false;}
   const int pending=submitted-(callback.delivered.load()-base_delivered);int peak=max_pending.load();while(pending>peak&&!max_pending.compare_exchange_weak(peak,pending)){}
   if(bounded&&pending>=8){restarts++;decoder->Release();if(!decoder->Configure(settings)){errors++;break;}decoder->RegisterDecodeCompleteCallback(&callback);submitted=0;base_delivered=callback.delivered.load();need_key=true;skipped++;continue;}
   webrtc::EncodedImage image;image.SetEncodedData(webrtc::EncodedImageBuffer::Create(packet.bytes.data(),packet.bytes.size()));image.SetRtpTimestamp(static_cast<uint32_t>(i*1500));image._frameType=packet.key?webrtc::VideoFrameType::kVideoFrameKey:webrtc::VideoFrameType::kVideoFrameDelta;
   if(decoder->Decode(image,false,0)!=WEBRTC_VIDEO_CODEC_OK){errors++;}else{submitted++;}
  }
  const auto deadline=Clock::now()+std::chrono::seconds(3);while(Clock::now()<deadline&&callback.delivered.load()-base_delivered<submitted)std::this_thread::sleep_for(std::chrono::milliseconds(10));
  decoder->Release();delivered+=callback.delivered.load();{std::lock_guard<std::mutex> lock(timing_mutex);ages.insert(ages.end(),callback.ages.begin(),callback.ages.end());}
 });
 for(auto& worker:workers)worker.join();std::sort(ages.begin(),ages.end());auto p=[&](double q){return ages.empty()?0:ages[size_t((ages.size()-1)*q)];};
 report<<"{\"schema\":1,\"completed\":true,\"hardware\":"<<(av1?"false":"true")<<",\"contexts\":"<<contexts<<",\"coresPerContext\":"<<cores<<",\"cpuMs\":"<<Cpu()-cpu<<",\"wallMs\":"<<Ms(Clock::now()-start)<<",\"inputPacketsPerContext\":"<<packets.size()<<",\"decoded\":"<<delivered<<",\"skippedForKeyframeRecovery\":"<<skipped<<",\"restarts\":"<<restarts<<",\"decodeErrors\":"<<errors<<",\"maxOutstandingFrames\":"<<max_pending<<",\"captureAgeMs\":{\"p50\":"<<p(.5)<<",\"p95\":"<<p(.95)<<",\"max\":"<<p(1)<<"},\"limits\":\"Actual pinned decoder, isolated fixture, no SFU/UI/device audio. 750ms synchronous output stall for hardware runs. Bounded prototype resets and waits for a fresh keyframe; skipped frames are explicitly reported. Not enabled in the app.\"}";
 return errors?4:0;
}
