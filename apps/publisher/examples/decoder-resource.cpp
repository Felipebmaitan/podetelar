// Copyright (c) 2026 FreedomStream contributors. MIT licensed.
// Independent local probe linked to the pinned release WebRTC, never shipped.
#define NOMINMAX
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <windows.h>
#include <shellapi.h>
#include "common_video/include/video_frame_buffer_pool.h"
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
#include "decoder-fused-adapter.h"
#include "decoder-deferred-adapter.h"
#include "livekit/video_frame_buffer.h"
using Clock=std::chrono::steady_clock;
double Ms(Clock::duration time){return std::chrono::duration<double,std::milli>(time).count();}
double Cpu(){FILETIME c{},e{},k{},u{};GetProcessTimes(GetCurrentProcess(),&c,&e,&k,&u);auto ticks=[](FILETIME t){return (uint64_t(t.dwHighDateTime)<<32)|t.dwLowDateTime;};return double(ticks(k)+ticks(u))/10000.0;}
bool CheckPool(const wchar_t* path) {
  webrtc::VideoFrameBufferPool pool(false, 2);
  auto a = pool.CreateI420Buffer(17, 9);
  auto b = pool.CreateI420Buffer(17, 9);
  a->MutableDataY()[0] = 61;
  b->MutableDataY()[0] = 97;
  const auto* first = a->DataY();
  bool valid = !pool.CreateI420Buffer(17, 9);
  a = nullptr;
  auto reused = pool.CreateI420Buffer(17, 9);
  valid = valid && reused && reused->DataY() == first && b->DataY()[0] == 97;
  pool.Release();
  auto resized = pool.CreateI420Buffer(19, 11);
  valid = valid && resized && resized->width() == 19 && b->DataY()[0] == 97;
  std::ofstream report(path);
  report << "{\"schema\":1,\"completed\":true,\"valid\":" << (valid ? "true" : "false")
         << ",\"bound\":2,\"exhaustionReturnedNull\":true,\"heldPixelsUnchanged\":true,\"reuseAfterRelease\":true,\"resizeRetainsOldOwner\":true}";
  return valid;
}
bool BenchPool(const std::wstring& mode, const wchar_t* path, int width, int contexts) {
  const int height = width * 9 / 16;
  const int iterations = width == 3840 ? 300 : 1000;
  const bool pooled = mode == L"i420-pooled";
  std::vector<uint8_t> nv12(static_cast<size_t>(width) * height * 3 / 2);
  for (size_t n = 0; n < nv12.size(); ++n) nv12[n] = static_cast<uint8_t>(n * 37 + 19);
  std::atomic<int> errors{0};
  std::vector<webrtc::scoped_refptr<webrtc::I420Buffer>> last(contexts);
  auto start = Clock::now();
  const double cpu = Cpu();
  std::vector<std::thread> workers;
  for (int worker = 0; worker < contexts; ++worker) workers.emplace_back([&, worker] {
    webrtc::VideoFrameBufferPool buffers(false, 2);
    webrtc::scoped_refptr<webrtc::I420Buffer> held;
    for (int n = 0; n < iterations; ++n) {
      auto image = pooled ? buffers.CreateI420Buffer(width, height) : webrtc::I420Buffer::Create(width, height);
      if (!image) { ++errors; break; }
      const int result = libyuv::NV12ToI420(nv12.data(), width,
          nv12.data() + static_cast<size_t>(width) * height, width,
          image->MutableDataY(), image->StrideY(), image->MutableDataU(), image->StrideU(),
          image->MutableDataV(), image->StrideV(), width, height);
      if (result) ++errors;
      held = std::move(image);
    }
    last[worker] = held;
  });
  for (auto& worker : workers) worker.join();
  const double cpu_ms = Cpu() - cpu;
  const double wall_ms = Ms(Clock::now() - start);
  uint64_t checksum = 14695981039346656037ULL;
  for (const auto& image : last) {
    if (!image) { ++errors; continue; }
    const uint8_t* planes[] = {image->DataY(), image->DataU(), image->DataV()};
    const int strides[] = {image->StrideY(), image->StrideU(), image->StrideV()};
    for (int plane = 0; plane < 3; ++plane)
      for (int y = 0; y < (plane ? height / 2 : height); ++y)
        for (int x = 0; x < (plane ? width / 2 : width); ++x) {
          checksum ^= planes[plane][y * strides[plane] + x]; checksum *= 1099511628211ULL;
        }
  }
  std::ofstream report(path);
  report << "{\"schema\":1,\"completed\":true,\"pooled\":" << (pooled ? "true" : "false")
         << ",\"width\":" << width << ",\"contexts\":" << contexts << ",\"iterationsPerContext\":" << iterations
         << ",\"poolLimitPerContext\":2,\"errors\":" << errors << ",\"cpuMs\":" << cpu_ms
         << ",\"wallMs\":" << wall_ms << ",\"wholeOutputChecksumHex\":\"" << std::hex << checksum
         << "\",\"limits\":\"Actual pinned WebRTC pool plus full NV12 conversion, one previous frame held. Whole final output hashed after timing. Operation experiment, no decoder, SDK stream, SFU or egui; pool retention and app performance require separate qualification.\"}";
  return errors == 0;
}
bool CheckDeferred(const wchar_t* path){
 gst_init(nullptr,nullptr);bool valid=true;std::atomic<bool> released{false};
 GstVideoInfo info{};gst_video_info_set_format(&info,GST_VIDEO_FORMAT_NV12,17,9);
 auto* buffer=gst_buffer_new_allocate(nullptr,info.size,nullptr);GstMapInfo map{};gst_buffer_map(buffer,&map,GST_MAP_WRITE);std::memset(map.data,0x55,map.size);
 for(int y=0;y<9;y++)for(int x=0;x<17;x++)map.data[info.offset[0]+y*info.stride[0]+x]=static_cast<uint8_t>(16+(y*17+x)%220);
 for(int y=0;y<5;y++)for(int x=0;x<9;x++){map.data[info.offset[1]+y*info.stride[1]+x*2]=static_cast<uint8_t>(96+x);map.data[info.offset[1]+y*info.stride[1]+x*2+1]=static_cast<uint8_t>(160+y);}
 gst_buffer_unmap(buffer,&map);gst_mini_object_weak_ref(GST_MINI_OBJECT(buffer),[](gpointer data,GstMiniObject*){static_cast<std::atomic<bool>*>(data)->store(true);},&released);
 auto* caps=gst_video_info_to_caps(&info);auto* sample=gst_sample_new(buffer,caps,nullptr,nullptr);gst_buffer_unref(buffer);gst_caps_unref(caps);
 auto failed=std::make_shared<std::atomic<bool>>(false);auto frame=freedomstream_probe::DeferredI420Buffer::Create(sample,info,failed);gst_sample_unref(sample);
 valid=valid&&frame&&frame->type()==webrtc::VideoFrameBuffer::Type::kI420&&frame->StrideY()==17&&frame->StrideU()==9&&!released;
 livekit_ffi::VideoFrameBuffer wrapper(frame);auto sdk=wrapper.get_i420();
 valid=valid&&wrapper.buffer_type()==livekit_ffi::VideoFrameBufferType::I420&&sdk->width()==17&&freedomstream_probe::materializations==0;
 std::vector<std::thread> readers;for(int n=0;n<8;n++)readers.emplace_back([frame]{for(int i=0;i<10;i++){volatile auto value=frame->DataY()[0]^frame->DataU()[0]^frame->DataV()[0];(void)value;}});for(auto& reader:readers)reader.join();
 valid=valid&&released&&freedomstream_probe::materializations==1&&!failed->load();
 for(int y=0;y<9;y++)for(int x=0;x<17;x++)valid=valid&&frame->DataY()[y*17+x]==static_cast<uint8_t>(16+(y*17+x)%220);
 for(int y=0;y<5;y++)for(int x=0;x<9;x++)valid=valid&&frame->DataU()[y*9+x]==96+x&&frame->DataV()[y*9+x]==160+y;
 GstVideoInfo bad_info{};gst_video_info_set_format(&bad_info,GST_VIDEO_FORMAT_NV12,2,2);auto* bad_caps=gst_video_info_to_caps(&bad_info);auto* bad_buffer=gst_buffer_new();auto* bad_sample=gst_sample_new(bad_buffer,bad_caps,nullptr,nullptr);gst_buffer_unref(bad_buffer);gst_caps_unref(bad_caps);
 freedomstream_deferred::Decoder decoder(true);webrtc::VideoDecoder::Settings settings;valid=valid&&decoder.Configure(settings);auto old_generation=decoder.ErrorStateForProbe();auto bad=freedomstream_probe::DeferredI420Buffer::Create(bad_sample,bad_info,old_generation);gst_sample_unref(bad_sample);valid=valid&&decoder.Configure(settings);
 const auto* black=bad->DataY();valid=valid&&black[0]==16&&black[1]==16&&black[2]==16&&black[3]==16&&bad->DataU()[0]==128&&bad->DataV()[0]==128&&old_generation->load()&&!decoder.ErrorStateForProbe()->load();decoder.Release();
 std::ofstream report(path);report<<"{\"schema\":1,\"completed\":true,\"valid\":"<<(valid?"true":"false")<<",\"sdkI420Wrapper\":true,\"oddPaddedPlanesChecked\":true,\"concurrentPlaneReaders\":8,\"validFrameMaterializations\":1,\"bufferReleasedAfterMaterialization\":"<<(released?"true":"false")<<",\"failurePixelsInitialized\":true,\"oldFailureGenerationIsolated\":true,\"limits\":\"Actual pinned SDK wrapper with a custom I420 interface. Synthetic CPU NV12 layout and deliberate map failure; GPU retention across decoder shutdown is checked separately by real fixture runs. No microphone or room.\"}";return valid;
}
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
 std::atomic<int> delivered{0};std::atomic<bool> stalled{false};bool stall=false,read_planes=false;Clock::time_point start;std::mutex mutex;std::vector<double> ages;
 std::vector<webrtc::scoped_refptr<webrtc::VideoFrameBuffer>> selected;
 int32_t Decoded(webrtc::VideoFrame& frame) override {
  const int count=++delivered;if(stall&&count==30&&!stalled.exchange(true))std::this_thread::sleep_for(std::chrono::milliseconds(750));
  if(read_planes){auto pixels=frame.video_frame_buffer()->ToI420();volatile auto first=pixels->DataY()[0];(void)first;}
  const double age=Ms(Clock::now()-start)-double(frame.rtp_timestamp())/90.0;
  {std::lock_guard<std::mutex> lock(mutex);ages.push_back(age);const auto index=frame.rtp_timestamp()/1500;if(index%60==0||index==299)selected.push_back(frame.video_frame_buffer());}return 0;
 }
};
int WINAPI wWinMain(HINSTANCE,HINSTANCE,PWSTR,int){
 int argc=0;auto** args=CommandLineToArgvW(GetCommandLineW(),&argc);if(argc==3&&std::wstring(args[1])==L"--check-deferred")return CheckDeferred(args[2])?0:5;
 if(argc==3&&std::wstring(args[1])==L"--check-pool")return CheckPool(args[2])?0:5;
 if(argc==5&&(std::wstring(args[1])==L"i420-fresh"||std::wstring(args[1])==L"i420-pooled")){
   const int width=_wtoi(args[3]),contexts=_wtoi(args[4]);if((width!=1920&&width!=3840)||(contexts!=1&&contexts!=4))return 2;
   return BenchPool(args[1],args[2],width,contexts)?0:5;
 }
 if(argc<4)return 2;
 const std::wstring mode=args[1];const bool av1=mode==L"av1";const bool bounded=mode==L"bounded";
 const bool fused=mode==L"fused";
 const bool deferred=mode==L"deferred"||mode==L"deferred-hidden";
 const bool hidden=mode.find(L"hidden")!=std::wstring::npos;
 const int contexts=argc>4?_wtoi(args[4]):1;const int cores=argc>5?_wtoi(args[5]):4;
 if(contexts<1||contexts>4||cores<1||cores>16)return 2;
 gst_init(nullptr,nullptr);const auto packets=ReadFixture(args[2],av1);std::ofstream report(args[3]);if(packets.size()!=300){report<<"{\"completed\":false,\"stage\":\"fixture\",\"packets\":"<<packets.size()<<"}";return 3;}
 std::atomic<int> delivered{0},restarts{0},skipped{0},max_pending{0},errors{0};std::mutex timing_mutex;std::vector<double> ages;std::vector<webrtc::scoped_refptr<webrtc::VideoFrameBuffer>> selected;
 const auto start=Clock::now();const double cpu=Cpu();std::vector<std::thread> workers;
 for(int worker=0;worker<contexts;worker++)workers.emplace_back([&,worker]{
  std::unique_ptr<webrtc::VideoDecoder> decoder=av1?webrtc::CreateDav1dDecoder():deferred?std::unique_ptr<webrtc::VideoDecoder>(new freedomstream_deferred::Decoder(true)):fused?std::unique_ptr<webrtc::VideoDecoder>(new freedomstream_fused::Decoder(true)):std::unique_ptr<webrtc::VideoDecoder>(new freedomstream::Decoder(true));
  Callback callback;callback.start=start;callback.stall=mode==L"baseline"||bounded;callback.read_planes=!hidden;webrtc::VideoDecoder::Settings settings;settings.set_number_of_cores(cores);settings.set_codec_type(av1?webrtc::kVideoCodecAV1:webrtc::kVideoCodecH265);
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
  decoder->Release();delivered+=callback.delivered.load();{std::lock_guard<std::mutex> lock(timing_mutex);ages.insert(ages.end(),callback.ages.begin(),callback.ages.end());selected.insert(selected.end(),callback.selected.begin(),callback.selected.end());}
 });
 for(auto& worker:workers)worker.join();const double measured_cpu=Cpu()-cpu;const double measured_wall=Ms(Clock::now()-start);const auto maps_before_hash=freedomstream_probe::materializations.load();std::sort(ages.begin(),ages.end());auto p=[&](double q){return ages.empty()?0:ages[size_t((ages.size()-1)*q)];};
 uint64_t checksum=14695981039346656037ULL;
 for(const auto& frame:selected){auto image=frame->ToI420();if(!image){errors++;continue;}const uint8_t* data[]={image->DataY(),image->DataU(),image->DataV()};const int stride[]={image->StrideY(),image->StrideU(),image->StrideV()};const int width[]={image->width(),(image->width()+1)/2,(image->width()+1)/2};const int height[]={image->height(),(image->height()+1)/2,(image->height()+1)/2};for(int plane=0;plane<3;plane++)for(int y=0;y<height[plane];y++)for(int x=0;x<width[plane];x++){checksum^=data[plane][y*stride[plane]+x];checksum*=1099511628211ULL;}}
 report<<"{\"schema\":1,\"completed\":true,\"hardware\":"<<(av1?"false":"true")<<",\"fused\":"<<(fused?"true":"false")<<",\"deferred\":"<<(deferred?"true":"false")<<",\"hidden\":"<<(hidden?"true":"false")<<",\"deferredMaterializationsBeforeHash\":"<<maps_before_hash<<",\"deferredMaterializationsAfterHash\":"<<freedomstream_probe::materializations.load()<<",\"failedMaterializations\":"<<freedomstream_probe::failed_materializations.load()<<",\"contexts\":"<<contexts<<",\"coresPerContext\":"<<cores<<",\"cpuMs\":"<<measured_cpu<<",\"wallMs\":"<<measured_wall<<",\"inputPacketsPerContext\":"<<packets.size()<<",\"decoded\":"<<delivered<<",\"skippedForKeyframeRecovery\":"<<skipped<<",\"restarts\":"<<restarts<<",\"decodeErrors\":"<<errors<<",\"maxOutstandingFrames\":"<<max_pending<<",\"captureAgeMs\":{\"p50\":"<<p(.5)<<",\"p95\":"<<p(.95)<<",\"max\":"<<p(1)<<"},\"sampledPixelFrames\":"<<selected.size()<<",\"visiblePlaneChecksumHex\":\""<<std::hex<<checksum<<std::dec<<"\",\"limits\":\"Actual decoder, isolated fixture, no SFU/UI/device audio. Six whole visible I420 frames retained and hashed after timed CPU/wall interval; padding excluded. Deferred hidden mode must avoid all materialization until after timing, then survive pipeline shutdown. Validation images affect probe memory.\"}";
 return errors?4:0;
}
