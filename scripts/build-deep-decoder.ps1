$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$out=Join-Path $root '.local/experiments/deep-pass/decoder'
& "$root/../freedomstream/.local/node24/node-v24.11.1-win-x64/node.exe" "$root/scripts/prepare-deep-decoder.mjs"
if($LASTEXITCODE -ne 0){throw 'Deep decoder preparation failed'}
$gst=Join-Path $root '.local/gstreamer'
$vs=& 'C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe' -latest -products '*' -property installationPath
$msvc=Get-ChildItem (Join-Path $vs 'VC/Tools/MSVC') -Directory|Sort-Object Name -Descending|Select-Object -First 1
$kit='C:/Program Files (x86)/Windows Kits/10';$sdk=Get-ChildItem "$kit/Include" -Directory|Sort-Object Name -Descending|Select-Object -First 1
$env:INCLUDE=@("$($msvc.FullName)/include","$($sdk.FullName)/ucrt","$($sdk.FullName)/um","$($sdk.FullName)/shared","$($sdk.FullName)/winrt") -join ';'
$env:LIB=@("$($msvc.FullName)/lib/x64","$kit/Lib/$($sdk.Name)/ucrt/x64","$kit/Lib/$($sdk.Name)/um/x64","$gst/lib") -join ';'
$env:PATH="$($msvc.FullName)/bin/Hostx64/x64;$env:PATH"
$scratch=Get-ChildItem "$root/target/release/build" -Directory -Filter 'scratch-*'|Where-Object {Test-Path "$($_.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release/include"}|Select-Object -First 1
$webrtc="$($scratch.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release"
$compilerArgs=@('/nologo','/O2','/EHsc','/MT','/std:c++20','/W4','/DNDEBUG','/DWEBRTC_WIN','/DGST_USE_UNSTABLE_API','/D_CRT_SECURE_NO_WARNINGS',"/I$out","/I$webrtc/include","/I$webrtc/include/third_party/abseil-cpp","/I$webrtc/include/third_party/libyuv/include","/I$gst/include/gstreamer-1.0","/I$gst/include/glib-2.0","/I$gst/lib/glib-2.0/include","/I$gst/lib/gstreamer-1.0/include","/Fo$out/deep-decoder.obj","/Fe$out/deep-decoder.exe","$root/apps/publisher/examples/deep-decoder.cpp",'/link','/SUBSYSTEM:WINDOWS',"$webrtc/lib/webrtc.lib",'gstreamer-1.0.lib','gstapp-1.0.lib','gstvideo-1.0.lib','gstd3d11-1.0.lib','glib-2.0.lib','gobject-2.0.lib','d3d11.lib','dxgi.lib','psapi.lib','shell32.lib','user32.lib','winmm.lib','ws2_32.lib','iphlpapi.lib','secur32.lib','crypt32.lib','ole32.lib','strmiids.lib','dmoguids.lib','msdmo.lib','wmcodecdspuuid.lib')
& "$($msvc.FullName)/bin/Hostx64/x64/cl.exe" @compilerArgs
if($LASTEXITCODE -ne 0){throw 'Deep decoder compilation failed'}
@{exeSha256=(Get-FileHash "$out/deep-decoder.exe").Hash;probeSha256=(Get-FileHash "$root/apps/publisher/examples/deep-decoder.cpp").Hash;source=(Get-Content "$out/source.json" -Raw|ConvertFrom-Json)}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath "$out/build.json" -Encoding utf8
