param([switch]$SdkIdle)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$output=Join-Path $root '.local/experiments/decoder';New-Item -ItemType Directory -Force -Path $output|Out-Null
$adapter=Get-Content -Raw -LiteralPath (Join-Path $root 'apps/publisher/decoder/freedomstream_decoder.h')
$factory=$adapter.IndexOf("`nclass DecoderFactory final")
if($factory -lt 0){throw 'Decoder adapter structure changed.'}
# Compile the exact application Decoder class, omitting its unrelated factory.
[IO.File]::WriteAllText((Join-Path $output 'decoder-probe-adapter.h'),$adapter.Substring(0,$factory)+"`n} // namespace freedomstream`n")
$gst=Join-Path $root '.local/gstreamer'
$vs=& 'C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe' -latest -products '*' -property installationPath
$msvc=Get-ChildItem (Join-Path $vs 'VC/Tools/MSVC') -Directory|Sort-Object Name -Descending|Select-Object -First 1
$kit='C:/Program Files (x86)/Windows Kits/10';$sdk=Get-ChildItem "$kit/Include" -Directory|Sort-Object Name -Descending|Select-Object -First 1
$env:INCLUDE=@("$($msvc.FullName)/include","$($sdk.FullName)/ucrt","$($sdk.FullName)/um","$($sdk.FullName)/shared","$($sdk.FullName)/winrt") -join ';'
$env:LIB=@("$($msvc.FullName)/lib/x64","$kit/Lib/$($sdk.Name)/ucrt/x64","$kit/Lib/$($sdk.Name)/um/x64","$gst/lib") -join ';'
$env:PATH="$($msvc.FullName)/bin/Hostx64/x64;$env:PATH"
$scratch=Get-ChildItem "$root/target/release/build" -Directory -Filter 'scratch-*'|Where-Object {Test-Path "$($_.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release/include"}|Select-Object -First 1
$webrtc="$($scratch.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release"
$arguments=@('/nologo','/O2','/EHsc','/MT','/std:c++20','/W4','/DNDEBUG','/DWEBRTC_WIN','/D_CRT_SECURE_NO_WARNINGS',"/I$output","/I$webrtc/include","/I$webrtc/include/third_party/abseil-cpp","/I$webrtc/include/third_party/libyuv/include","/I$gst/include/gstreamer-1.0","/I$gst/include/glib-2.0","/I$gst/lib/glib-2.0/include","/Fo$output/decoder-resource.obj","/Fe$output/decoder-resource.exe","$root/apps/publisher/examples/decoder-resource.cpp",'/link','/SUBSYSTEM:WINDOWS',"$webrtc/lib/webrtc.lib",'gstreamer-1.0.lib','gstapp-1.0.lib','gstvideo-1.0.lib','glib-2.0.lib','gobject-2.0.lib','d3d11.lib','dxgi.lib','shell32.lib','user32.lib','winmm.lib','ws2_32.lib','iphlpapi.lib','secur32.lib','crypt32.lib','ole32.lib','strmiids.lib','dmoguids.lib','msdmo.lib','wmcodecdspuuid.lib')
& "$($msvc.FullName)/bin/Hostx64/x64/cl.exe" @arguments
if($LASTEXITCODE -ne 0){throw 'Decoder experiment compilation failed.'}
@{adapterSha256=(Get-FileHash "$root/apps/publisher/decoder/freedomstream_decoder.h").Hash;probeSha256=(Get-FileHash "$root/apps/publisher/examples/decoder-resource.cpp").Hash;executableSha256=(Get-FileHash "$output/decoder-resource.exe").Hash;decoderPin=(Get-Content -Raw "$root/deploy/native-decoder.json"|ConvertFrom-Json)}|ConvertTo-Json -Depth 8|Set-Content "$output/source.json"
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
