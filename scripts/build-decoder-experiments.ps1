param([switch]$SdkIdle, [switch]$BoundedReadback)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$output=Join-Path $root '.local/experiments/decoder';New-Item -ItemType Directory -Force -Path $output|Out-Null
$adapter=(& git -C $root show '93d6ee13d72898fa52b22305ff0f64e7ea05aea2:apps/publisher/decoder/freedomstream_decoder.h') -join "`n"
$factory=$adapter.IndexOf("`nclass DecoderFactory final")
if($factory -lt 0){throw 'Decoder adapter structure changed.'}
# Compile the exact application Decoder class, omitting its unrelated factory.
[IO.File]::WriteAllText((Join-Path $output 'decoder-probe-adapter.h'),$adapter.Substring(0,$factory)+"`n} // namespace freedomstream`n")
$fused=$adapter.Substring(0,$factory).Replace('namespace freedomstream {','namespace freedomstream_fused {').Replace('#include "libyuv/planar_functions.h"','#include "libyuv/planar_functions.h"'+"`n"+'#include "libyuv/convert.h"')
$oldCaps='! d3d11download ! videoconvert ! video/x-raw,format=I420'
if(($fused.Split($oldCaps).Count-1) -ne 3){throw 'Expected three original receive pipelines.'}
$fused=$fused.Replace($oldCaps,'! d3d11download ! video/x-raw,format=NV12')
$fused=$fused.Replace('!gst_video_frame_map(&mapped, &info, buffer, GST_MAP_READ)', 'GST_VIDEO_INFO_FORMAT(&info) != GST_VIDEO_FORMAT_NV12 || !gst_video_frame_map(&mapped, &info, buffer, GST_MAP_READ)')
$start=$fused.IndexOf('    libyuv::I420Copy(');$end=$fused.IndexOf(');',$start)+2
if($start -lt 0 -or $end -le $start){throw 'Original I420 copy marker changed.'}
$replacement=@'
    const int converted = libyuv::NV12ToI420(
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 0)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 0),
      static_cast<const uint8_t*>(GST_VIDEO_FRAME_PLANE_DATA(&mapped, 1)), GST_VIDEO_FRAME_PLANE_STRIDE(&mapped, 1),
      pixels->MutableDataY(), pixels->StrideY(), pixels->MutableDataU(), pixels->StrideU(),
      pixels->MutableDataV(), pixels->StrideV(), info.width, info.height);
    if (converted != 0) { gst_video_frame_unmap(&mapped); gst_sample_unref(sample); return GST_FLOW_ERROR; }
'@
$fused=$fused.Substring(0,$start)+$replacement+$fused.Substring($end)+"`n} // namespace freedomstream_fused`n"
[IO.File]::WriteAllText((Join-Path $output 'decoder-fused-adapter.h'),$fused)
$deferred=$adapter.Substring(0,$factory).Replace('namespace freedomstream {','namespace freedomstream_deferred {')
$deferred=$deferred.Replace($oldCaps,'! video/x-raw(memory:D3D11Memory),format=NV12')
$deferred=$deferred.Replace('#include <atomic>',"#include <atomic>`n#include `"deferred-i420.h`"")
$deferred=$deferred.Replace('    Stop();'+"`n"+'    const char* pipeline', '    Stop();'+"`n"+'    failed_generation_ = std::make_shared<std::atomic<bool>>(false);'+"`n"+'    const char* pipeline')
$deferred=$deferred.Replace('    const size_t prefix = av1_ ? 2 : 0;', '    if (failed_generation_->load(std::memory_order_acquire)) return WEBRTC_VIDEO_CODEC_ERROR;'+"`n"+'    const size_t prefix = av1_ ? 2 : 0;')
$deferred=$deferred.Replace('    GstVideoFrame mapped;','')
$deferred=$deferred.Replace('!gst_video_frame_map(&mapped, &info, buffer, GST_MAP_READ)', 'GST_VIDEO_INFO_FORMAT(&info) != GST_VIDEO_FORMAT_NV12')
$start=$deferred.IndexOf('    auto pixels = webrtc::I420Buffer::Create');$end=$deferred.IndexOf('    uint32_t timestamp = 0;',$start)
if($start -lt 0 -or $end -le $start){throw 'Deferred frame lease marker changed.'}
$replacement='    auto pixels = freedomstream_probe::DeferredI420Buffer::Create(sample, info, failed_generation_);'+"`n"+'    if (!pixels) { gst_sample_unref(sample); return GST_FLOW_ERROR; }'+"`n"
$deferred=$deferred.Substring(0,$start)+$replacement+$deferred.Substring($end)
$deferred=$deferred.Replace('  const bool hevc_;','  std::shared_ptr<std::atomic<bool>> failed_generation_ = std::make_shared<std::atomic<bool>>(false);'+"`n"+'  const bool hevc_;')
$deferred=$deferred.Replace('  explicit Decoder(', '  std::shared_ptr<std::atomic<bool>> ErrorStateForProbe() const { return failed_generation_; }'+"`n"+'  explicit Decoder(')
[IO.File]::WriteAllText((Join-Path $output 'decoder-deferred-adapter.h'),$deferred+"`n} // namespace freedomstream_deferred`n")
if ($BoundedReadback) {
 $deferred=$deferred.Replace('sample, info, failed_generation_);','sample, info, failed_generation_, readback_);')
 $deferred=$deferred.Replace('    failed_generation_ = std::make_shared<std::atomic<bool>>(false);', '    failed_generation_ = std::make_shared<std::atomic<bool>>(false);' + "`n" + '    readback_ = std::make_shared<freedomstream_probe::BoundedReadback>();')
 $deferred=$deferred.Replace('  const bool hevc_;', '  std::shared_ptr<freedomstream_probe::BoundedReadback> readback_ = std::make_shared<freedomstream_probe::BoundedReadback>();' + "`n" + '  const bool hevc_;')
 [IO.File]::WriteAllText((Join-Path $output 'decoder-deferred-adapter.h'),$deferred+"`n} // namespace freedomstream_deferred`n")
}
Copy-Item -LiteralPath "$root/apps/publisher/examples/deferred-i420.h" -Destination "$output/deferred-i420.h" -Force
Copy-Item -LiteralPath "$root/apps/publisher/decoder/deferred_i420.h" -Destination "$output/deferred_i420.h" -Force
if ($BoundedReadback) {
 Copy-Item -LiteralPath "$root/scripts/experiments/bounded-deferred-i420.h" -Destination "$output/deferred_i420.h" -Force
 Copy-Item -LiteralPath "$root/scripts/experiments/bounded-readback.h" -Destination "$output/bounded-readback.h" -Force
}
$gst=Join-Path $root '.local/gstreamer'
$vs=& 'C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe' -latest -products '*' -property installationPath
$msvc=Get-ChildItem (Join-Path $vs 'VC/Tools/MSVC') -Directory|Sort-Object Name -Descending|Select-Object -First 1
$kit='C:/Program Files (x86)/Windows Kits/10';$sdk=Get-ChildItem "$kit/Include" -Directory|Sort-Object Name -Descending|Select-Object -First 1
$env:INCLUDE=@("$($msvc.FullName)/include","$($sdk.FullName)/ucrt","$($sdk.FullName)/um","$($sdk.FullName)/shared","$($sdk.FullName)/winrt") -join ';'
$env:LIB=@("$($msvc.FullName)/lib/x64","$kit/Lib/$($sdk.Name)/ucrt/x64","$kit/Lib/$($sdk.Name)/um/x64","$gst/lib") -join ';'
$env:PATH="$($msvc.FullName)/bin/Hostx64/x64;$env:PATH"
$scratch=Get-ChildItem "$root/target/release/build" -Directory -Filter 'scratch-*'|Where-Object {Test-Path "$($_.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release/include"}|Select-Object -First 1
$webrtc="$($scratch.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release"
$sys=Get-ChildItem "$root/target/release/build" -Directory -Filter 'webrtc-sys-*'|Where-Object {Test-Path "$($_.FullName)/out/webrtcsys-cxx.lib"}|Select-Object -First 1
$arguments=@('/nologo','/O2','/EHsc','/MT','/std:c++20','/W4','/DNDEBUG','/DWEBRTC_WIN','/D_CRT_SECURE_NO_WARNINGS',"/I$output","/I$webrtc/include","/I$webrtc/include/third_party/abseil-cpp","/I$webrtc/include/third_party/libyuv/include","/I$gst/include/gstreamer-1.0","/I$gst/include/glib-2.0","/I$gst/lib/glib-2.0/include","/Fo$output/decoder-resource.obj","/Fe$output/decoder-resource.exe","$root/apps/publisher/examples/decoder-resource.cpp",'/link','/SUBSYSTEM:WINDOWS',"$webrtc/lib/webrtc.lib",'gstreamer-1.0.lib','gstapp-1.0.lib','gstvideo-1.0.lib','glib-2.0.lib','gobject-2.0.lib','d3d11.lib','dxgi.lib','shell32.lib','user32.lib','winmm.lib','ws2_32.lib','iphlpapi.lib','secur32.lib','crypt32.lib','ole32.lib','strmiids.lib','dmoguids.lib','msdmo.lib','wmcodecdspuuid.lib')
$linkIndex=[array]::IndexOf($arguments,'/link')
if ($BoundedReadback) {
 $arguments=@($arguments[0..($linkIndex-1)])+@("/I$gst/lib/gstreamer-1.0/include",'/DGST_USE_UNSTABLE_API')+@($arguments[$linkIndex..($arguments.Length-1)])+@('gstd3d11-1.0.lib')
 $linkIndex=[array]::IndexOf($arguments,'/link')
}
$arguments=@($arguments[0..($linkIndex-1)])+@("/I$root/.local/native-sdk/webrtc-sys/include","/I$($sys.FullName)/out/cxxbridge/include")+@($arguments[$linkIndex..($arguments.Length-1)])+@("$($sys.FullName)/out/webrtcsys-cxx.lib")
& "$($msvc.FullName)/bin/Hostx64/x64/cl.exe" @arguments
if($LASTEXITCODE -ne 0){throw 'Decoder experiment compilation failed.'}
@{decoderClassSourceCommit='93d6ee13d72898fa52b22305ff0f64e7ea05aea2';boundedReadback=[bool]$BoundedReadback;deferredHeaderSha256=(Get-FileHash "$output/deferred_i420.h").Hash;readbackHeaderSha256=$(if($BoundedReadback){(Get-FileHash "$output/bounded-readback.h").Hash}else{$null});generatedAdapterSha256=(Get-FileHash "$output/decoder-deferred-adapter.h").Hash;adapterSha256=(Get-FileHash "$root/apps/publisher/decoder/freedomstream_decoder.h").Hash;probeSha256=(Get-FileHash "$root/apps/publisher/examples/decoder-resource.cpp").Hash;executableSha256=(Get-FileHash "$output/decoder-resource.exe").Hash;decoderPin=(Get-Content -Raw "$root/deploy/native-decoder.json"|ConvertFrom-Json)}|ConvertTo-Json -Depth 8|Set-Content "$output/source.json"
if($SdkIdle){
 $original=Get-Content -Raw "$root/.local/native-sdk/webrtc-sys/src/synthetic_audio_device.cpp"
 $marker='[this]() {'
 if(($original.Split($marker).Count-1) -ne 1){throw 'Synthetic ADM task marker changed.'}
 $instrumented=$original.Replace('namespace livekit_ffi {',"std::atomic<uint64_t> freedomstream_probe_ticks{0};`nnamespace livekit_ffi {").Replace($marker,$marker+' freedomstream_probe_ticks.fetch_add(1, std::memory_order_relaxed);')
 [IO.File]::WriteAllText("$output/synthetic-probe.cpp",$instrumented)
 $idleArgs=@('/nologo','/O2','/EHsc','/MT','/std:c++20','/W4','/DNDEBUG','/DWEBRTC_WIN',"/I$output","/I$root/.local/native-sdk/webrtc-sys/include","/I$webrtc/include","/I$webrtc/include/third_party/abseil-cpp","/Fo$output/sdk-idle.obj","/Fe$output/sdk-idle.exe","$root/apps/publisher/examples/sdk-idle-resource.cpp",'/link','/SUBSYSTEM:WINDOWS',"$webrtc/lib/webrtc.lib",'shell32.lib','user32.lib','winmm.lib','ws2_32.lib','iphlpapi.lib','secur32.lib','crypt32.lib','ole32.lib','strmiids.lib','dmoguids.lib','msdmo.lib','wmcodecdspuuid.lib')
 & "$($msvc.FullName)/bin/Hostx64/x64/cl.exe" @idleArgs
 if($LASTEXITCODE -ne 0){throw 'SDK idle probe compilation failed.'}
}
