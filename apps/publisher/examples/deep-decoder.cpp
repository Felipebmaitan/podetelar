// MIT. Actual current Decoder class with isolated diagnostics/variant switches.
#define NOMINMAX
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <windows.h>
#include <shellapi.h>
#include <psapi.h>
#include <algorithm>
#include <barrier>
#include <fstream>
#include <sstream>
#include <thread>
#include <vector>
#include "api/video_codecs/video_decoder.h"
#include "deep-adapter.h"
using Clock=std::chrono::steady_clock;
double Ms(Clock::duration d){return std::chrono::duration<double,std::milli>(d).count();}
double Cpu(){FILETIME c{},e{},k{},u{};GetProcessTimes(GetCurrentProcess(),&c,&e,&k,&u);auto ticks=[](FILETIME t){return (uint64_t(t.dwHighDateTime)<<32)|t.dwLowDateTime;};return double(ticks(k)+ticks(u))/10000.0;}
std::string Memory(){PROCESS_MEMORY_COUNTERS_EX m{};GetProcessMemoryInfo(GetCurrentProcess(),reinterpret_cast<PROCESS_MEMORY_COUNTERS*>(&m),sizeof(m));std::ostringstream s;s<<"{\"privateBytes\":"<<m.PrivateUsage<<",\"workingSetBytes\":"<<m.WorkingSetSize<<"}";return s.str();}
std::string Stats(){uint64_t values[27]{};deep::Snapshot(values,27);const char* names[]={"devicesCreated","devicesLive","samplesLive","samplesPeak","deferredLive","planeBytesLive","stagingLive","readbacks","readbackLockNs","deviceLockNs","copySubmitNs","mapWallNs","conversionNs","mapCalls","mapBusy","blockingFallbacks","materializations","failures","arraySamples","singleSamples","boundedSuccess","genericFallback","mapErrors","directRenderLogs","outputCopyLogs","poolFullLogs","traceMessages"};std::ostringstream s;s<<"{";for(int i=0;i<27;i++){if(i)s<<",";s<<"\""<<names[i]<<"\":"<<values[i];}s<<"}";return s.str();}
struct Packet{std::vector<uint8_t> bytes;bool key;};
std::vector<Packet> Fixture(const wchar_t* path,const std::string& codec){
 auto* filename=g_utf16_to_utf8(reinterpret_cast<const gunichar2*>(path),-1,nullptr,nullptr,nullptr);
 std::string description="filesrc name=source ! "+codec+"parse ! video/x-"+codec+(codec=="av1"?",stream-format=obu-stream,alignment=tu":",stream-format=byte-stream,alignment=au")+" ! appsink name=out sync=false max-buffers=4 drop=false";
 auto* pipeline=gst_parse_launch(description.c_str(),nullptr);if(!pipeline){g_free(filename);return {};}
 auto* source=gst_bin_get_by_name(GST_BIN(pipeline),"source");g_object_set(source,"location",filename,nullptr);g_free(filename);gst_object_unref(source);
 auto* sink=gst_bin_get_by_name(GST_BIN(pipeline),"out");gst_element_set_state(pipeline,GST_STATE_PLAYING);
 std::vector<Packet> packets;size_t bytes=0;const auto start=Clock::now();
 while(Ms(Clock::now()-start)<10000&&packets.size()<601){auto* sample=gst_app_sink_try_pull_sample(GST_APP_SINK(sink),50*GST_MSECOND);if(!sample){if(gst_app_sink_is_eos(GST_APP_SINK(sink)))break;continue;}GstMapInfo map{};auto* b=gst_sample_get_buffer(sample);if(gst_buffer_map(b,&map,GST_MAP_READ)){bytes+=map.size;if(bytes<64*1024*1024){const size_t prefix=codec=="av1"&&map.size>=2&&map.data[0]==0x12&&map.data[1]==0?2:0;packets.push_back({std::vector<uint8_t>(map.data+prefix,map.data+map.size),!GST_BUFFER_FLAG_IS_SET(b,GST_BUFFER_FLAG_DELTA_UNIT)});}gst_buffer_unmap(b,&map);}gst_sample_unref(sample);}
 gst_element_set_state(pipeline,GST_STATE_NULL);gst_object_unref(sink);gst_object_unref(pipeline);return packets;
}
uint64_t Hash(const webrtc::I420BufferInterface* image){uint64_t h=14695981039346656037ULL;const uint8_t* data[]={image->DataY(),image->DataU(),image->DataV()};const int strides[]={image->StrideY(),image->StrideU(),image->StrideV()};const int w[]={image->width(),(image->width()+1)/2,(image->width()+1)/2},height[]={image->height(),(image->height()+1)/2,(image->height()+1)/2};for(int p=0;p<3;p++)for(int y=0;y<height[p];y++)for(int x=0;x<w[p];x++){h^=data[p][y*strides[p]+x];h*=1099511628211ULL;}return h;}
struct Callback final:webrtc::DecodedImageCallback {
 std::atomic<int> count{0};bool hidden=false,hash_all=false;Clock::time_point start;
 std::mutex mutex;std::vector<double> age;std::vector<uint64_t> hashes;
 std::vector<webrtc::scoped_refptr<webrtc::VideoFrameBuffer>> selected;
 int32_t Decoded(webrtc::VideoFrame& frame)override{
  auto owner=frame.video_frame_buffer();
  if(!hidden||hash_all){auto i=owner->ToI420();volatile auto y=i->DataY()[0];(void)y;if(hash_all)hashes.push_back(Hash(i.get()));}
  {std::lock_guard<std::mutex> lock(mutex);age.push_back(Ms(Clock::now()-start)-frame.rtp_timestamp()/90.0);const auto index=frame.rtp_timestamp()/1500;if(index%60==0||index==299)selected.push_back(owner);}
  count++;return 0;
 }
};
int WINAPI wWinMain(HINSTANCE,HINSTANCE,PWSTR,int){
 int argc=0;auto** args=CommandLineToArgvW(GetCommandLineW(),&argc);if(argc<7)return 2;
 const int contexts=_wtoi(args[3]),cycles=_wtoi(args[4]);const bool hidden=std::wstring(args[5])==L"hidden";const std::wstring codec_arg=args[6];const std::string codec(codec_arg.begin(),codec_arg.end());
 if((contexts!=1&&contexts!=4)||cycles<1||cycles>10||(codec!="h265"&&codec!="h264"&&codec!="av1"))return 2;
 gst_init(nullptr,nullptr);const auto packets=Fixture(args[1],codec);if(packets.size()!=300)return 3;
 const bool hash_all=deep::Flag("FREEDOMSTREAM_DEEP_HASH_ALL");std::ofstream report(args[2]);
 report<<"{\"schema\":1,\"contexts\":"<<contexts<<",\"cycles\":"<<cycles<<",\"sharedDevice\":"<<(deep::Shared()?"true":"false")<<",\"relaxedMap\":"<<(deep::Relaxed()?"true":"false")<<",\"flushedPolling\":"<<(deep::FlushedPolling()?"true":"false")<<",\"allFrameHashQualification\":"<<(hash_all?"true":"false")<<",\"hidden\":"<<(hidden?"true":"false")<<",\"codec\":\""<<codec<<"\",\"runs\":[";
 int failures=0;
 for(int cycle=0;cycle<cycles;cycle++){
  std::vector<Callback> callbacks(contexts);std::vector<std::thread> workers;std::atomic<int> errors{0};Clock::time_point start;std::barrier gate(contexts,[&]() noexcept {start=Clock::now();});
  const auto before=Stats();const auto cpu=Cpu();const auto wall=Clock::now();
  for(int j=0;j<contexts;j++)workers.emplace_back([&,j]{
   freedomstream::Decoder decoder(codec=="h265",codec=="av1");auto& cb=callbacks[j];cb.hidden=hidden;cb.hash_all=hash_all;
   webrtc::VideoDecoder::Settings settings;
   const bool configured=decoder.Configure(settings);if(!configured)errors++;
   decoder.RegisterDecodeCompleteCallback(&cb);gate.arrive_and_wait();cb.start=start;
   if(!configured)return;
   for(size_t i=0;i<packets.size();i++){
    std::this_thread::sleep_until(start+std::chrono::microseconds(i*1000000/60));
    const auto& p=packets[i];webrtc::EncodedImage image;image.SetEncodedData(webrtc::EncodedImageBuffer::Create(p.bytes.data(),p.bytes.size()));image.SetRtpTimestamp(static_cast<uint32_t>(i*1500));image._frameType=p.key?webrtc::VideoFrameType::kVideoFrameKey:webrtc::VideoFrameType::kVideoFrameDelta;
    if(decoder.Decode(image,false,0)!=WEBRTC_VIDEO_CODEC_OK)errors++;
   }
   const auto deadline=Clock::now()+std::chrono::seconds(5);while(cb.count.load()<300&&Clock::now()<deadline)std::this_thread::sleep_for(std::chrono::milliseconds(1));decoder.Release();
  });
  for(auto& worker:workers)worker.join();const double cpu_ms=Cpu()-cpu,wall_ms=Ms(Clock::now()-wall);
  const auto after=Stats(),active_memory=Memory();std::vector<double> ages;int delivered=0;std::vector<uint64_t> sampled;
  for(auto& cb:callbacks){delivered+=cb.count.load();ages.insert(ages.end(),cb.age.begin(),cb.age.end());for(auto& frame:cb.selected){auto i=frame->ToI420();sampled.push_back(Hash(i.get()));}}
  std::sort(ages.begin(),ages.end());auto percentile=[&](double p){return ages.empty()?0:ages[size_t((ages.size()-1)*p)];};
  uint64_t checksum=14695981039346656037ULL;for(auto h:sampled){checksum^=h;checksum*=1099511628211ULL;}
  if(cycle)report<<",";
  report<<"{\"cycle\":"<<cycle<<",\"decoded\":"<<delivered<<",\"errors\":"<<errors<<",\"cpuMs\":"<<cpu_ms<<",\"wallMs\":"<<wall_ms<<",\"ageP50Ms\":"<<percentile(.5)<<",\"ageP95Ms\":"<<percentile(.95)<<",\"ageMaxMs\":"<<percentile(1)<<",\"sampledFrames\":"<<sampled.size()<<",\"sampledWholePlaneHash\":\""<<std::hex<<checksum<<std::dec<<"\",\"before\":"<<before<<",\"afterTimed\":"<<after<<",\"activeMemory\":"<<active_memory<<",\"allHashes\":[";
  bool first=true;for(auto& cb:callbacks)for(auto h:cb.hashes){if(!first)report<<",";first=false;report<<"\""<<std::hex<<h<<std::dec<<"\"";}
  report<<"]";for(auto& cb:callbacks)cb.selected.clear();std::this_thread::sleep_for(std::chrono::milliseconds(250));
  report<<",\"settled\":"<<Stats()<<",\"settledMemory\":"<<Memory()<<"}";report.flush();
  if(delivered!=contexts*300||errors||deep::failures.load())failures++;
 }
 report<<"],\"completed\":"<<(failures?"false":"true")<<",\"limits\":\"Actual current Decoder in isolated paced fixture; no SFU, egui or physical presentation. Timed CPU includes Configure/Release. Six validation frames/context retained and hashed after timing; active memory includes those owners. Hash-all qualification is not a performance run.\"}";
 return failures?4:0;
}
