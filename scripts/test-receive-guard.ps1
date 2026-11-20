# Compile the guard regression against the exact native headers/lib extracted by Cargo.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$relative = 'out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release'
$webrtc = Get-ChildItem (Join-Path $target 'debug/build') -Directory -Filter 'scratch-*' |
    ForEach-Object { Join-Path $_.FullName $relative } |
    Where-Object { Test-Path -LiteralPath (Join-Path $_ 'lib/webrtc.lib') } |
    Select-Object -First 1
if (-not $webrtc) { throw 'Build the pinned WebRTC SDK with Cargo before running the receive guard test.' }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$msvc = Get-ChildItem "$vs/VC/Tools/MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1
$kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10'
$sdk = Get-ChildItem "$kits/Include" -Directory | Sort-Object Name -Descending | Select-Object -First 1
$env:INCLUDE = @("$($msvc.FullName)/include", "$($sdk.FullName)/ucrt", "$($sdk.FullName)/um", "$($sdk.FullName)/shared") -join ';'
$env:LIB = @("$($msvc.FullName)/lib/x64", "$kits/Lib/$($sdk.Name)/ucrt/x64", "$kits/Lib/$($sdk.Name)/um/x64") -join ';'
$bin = "$($msvc.FullName)/bin/Hostx64/x64"
$env:PATH = "$bin;$env:PATH"
$output = Join-Path $repo '.local/receive-guard-test'
New-Item -ItemType Directory -Force -Path $output | Out-Null
& "$bin/cl.exe" /nologo /O2 /EHsc /MT /std:c++20 /DNDEBUG /DNOMINMAX /DWEBRTC_WIN "/I$repo/apps/publisher/decoder" "/I$webrtc/include" "/I$webrtc/include/third_party/abseil-cpp" "/Fo$output/test.obj" "/Fe$output/test.exe" "$repo/apps/publisher/sdk/receive-guard-test.cpp" /link "$webrtc/lib/webrtc.lib" user32.lib winmm.lib ws2_32.lib iphlpapi.lib secur32.lib crypt32.lib ole32.lib strmiids.lib dmoguids.lib msdmo.lib wmcodecdspuuid.lib
if ($LASTEXITCODE) { throw 'Could not build the native receive guard regression.' }
& "$output/test.exe"
if ($LASTEXITCODE) { throw 'Native receive guard regression failed.' }
Write-Output 'Protected audio/video receive configs drop until an enforcing transform is registered.'
