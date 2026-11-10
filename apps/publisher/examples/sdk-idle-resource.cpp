// Copyright (c) 2026 FreedomStream contributors. MIT licensed.
// Compile-only inclusion below preserves the upstream file's Apache notice.
#define NOMINMAX
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <algorithm>
#include <chrono>
#include <cstring>
#include <fstream>
#include <mutex>
#include <thread>
#include <vector>
#include "api/environment/environment_factory.h"
#include "api/make_ref_counted.h"
#include "synthetic-probe.cpp"
using Clock=std::chrono::steady_clock;
double Millis(Clock::duration value){return std::chrono::duration<double,std::milli>(value).count();}
double CpuMs(){FILETIME c{},e{},k{},u{};GetProcessTimes(GetCurrentProcess(),&c,&e,&k,&u);auto ticks=[](FILETIME t){return (uint64_t(t.dwHighDateTime)<<32)|t.dwLowDateTime;};return double(ticks(k)+ticks(u))/10000.0;}
class Transport final:public webrtc::AudioTransport {
 public:
 std::atomic<int> calls{0},recorded{0};std::mutex mutex;std::vector<double> intervals;Clock::time_point last{};
 int32_t RecordedDataIsAvailable(const void*,size_t,size_t,size_t,uint32_t,uint32_t,int32_t,uint32_t,bool,uint32_t& level)override{recorded++;level=0;return 0;}
 int32_t NeedMorePlayData(size_t samples,size_t bytes,size_t,uint32_t,void* data,size_t& out,int64_t* elapsed,int64_t* ntp)override{
  std::memset(data,0,samples*bytes);out=samples;*elapsed=0;*ntp=0;calls++;
  const auto now=Clock::now();std::lock_guard<std::mutex> lock(mutex);if(last!=Clock::time_point{}){const auto interval=Millis(now-last);if(interval<100)intervals.push_back(interval);}last=now;return 0;
 }
 void PullRenderData(int bits,int,size_t channels,size_t frames,void* data,int64_t* elapsed,int64_t* ntp)override{std::memset(data,0,frames*channels*(bits/8));*elapsed=0;*ntp=0;}
};
int WINAPI wWinMain(HINSTANCE,HINSTANCE,PWSTR,int){
 int argc=0;auto** args=CommandLineToArgvW(GetCommandLineW(),&argc);if(argc!=3)return 2;const bool gated=std::wstring(args[1])==L"gated";std::ofstream report(args[2]);
 const auto env=webrtc::CreateEnvironment();auto device=webrtc::make_ref_counted<livekit_ffi::SyntheticAudioDevice>(env);Transport transport;device->RegisterAudioCallback(&transport);device->Init();
 const auto start=Clock::now();const auto cpu=CpuMs();std::vector<uint64_t> ticks;std::vector<int> callbacks;
 for(int phase=0;phase<4;phase++){
  if(phase%2){if(gated)device->Init();device->StartPlayout();}else{device->StopPlayout();if(gated)device->Terminate();}
  const auto before=freedomstream_probe_ticks.load();const auto calls=transport.calls.load();std::this_thread::sleep_for(std::chrono::seconds(2));ticks.push_back(freedomstream_probe_ticks.load()-before);callbacks.push_back(transport.calls.load()-calls);
 }
 device->StopPlayout();device->Terminate();std::sort(transport.intervals.begin(),transport.intervals.end());auto p=[&](double q){return transport.intervals.empty()?0:transport.intervals[size_t((transport.intervals.size()-1)*q)];};
 const bool valid=callbacks[0]==0&&callbacks[2]==0&&callbacks[1]>=180&&callbacks[3]>=180&&transport.recorded==0;
 report<<"{\"schema\":1,\"completed\":true,\"valid\":"<<(valid?"true":"false")<<",\"gated\":"<<(gated?"true":"false")<<",\"wallMs\":"<<Millis(Clock::now()-start)<<",\"cpuMs\":"<<CpuMs()-cpu<<",\"idleTicks\":"<<ticks[0]+ticks[2]<<",\"playingTicks\":"<<ticks[1]+ticks[3]<<",\"playingCallbacks\":"<<callbacks[1]+callbacks[3]<<",\"captureCallbacks\":"<<transport.recorded<<",\"playoutIntervalMs\":{\"p50\":"<<p(.5)<<",\"p95\":"<<p(.95)<<",\"max\":"<<p(1)<<"},\"limits\":\"Actual pinned synthetic ADM with a counter in its repeating task. Gated prototype terminates only while stopped and reinitializes before playout. In-memory silent transport; no speaker or microphone, SFU or audio-receive integration claim.\"}";return valid?0:3;
}
