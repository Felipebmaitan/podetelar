param([ValidatePattern('^[a-z0-9-]{1,20}$')][string]$Label='sdk-v2',[switch]$ObserveGl)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
if((Test-Path -LiteralPath "$root/.local/experiments/deep-pass/$Label-room.exe")-or(Test-Path -LiteralPath "$root/.local/experiments/deep-pass/source-$Label")){throw 'Choose a new immutable build label'}
$node=Join-Path $root '../freedomstream/.local/node24/node-v24.11.1-win-x64/node.exe'
& $node "$root/scripts/setup-native-decoder.mjs"
if($LASTEXITCODE -ne 0){throw 'Original SDK verification failed'}
& $node "$root/scripts/prepare-deep-decoder.mjs"
if($LASTEXITCODE -ne 0){throw 'Experimental header preparation failed'}
$target=[IO.Path]::GetFullPath("$root/.local/experiments/deep-pass/sdk/webrtc-sys")
$boundary=[IO.Path]::GetFullPath("$root/.local/experiments/deep-pass/")
if(-not $target.StartsWith($boundary,[StringComparison]::OrdinalIgnoreCase)){throw 'Unexpected SDK destination'}
if(-not(Test-Path -LiteralPath $target)){New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target)|Out-Null;Copy-Item -LiteralPath "$root/.local/native-sdk/webrtc-sys" -Destination $target -Recurse}
foreach($name in @('freedomstream_decoder.h','deferred_i420.h','bounded_readback.h','deep-decoder-support.h')){Copy-Item -LiteralPath "$root/.local/experiments/deep-pass/decoder/$name" -Destination "$target/src/$name"}
$build=Get-Content -LiteralPath "$target/build.rs" -Raw
if(-not $build.Contains('cargo:rerun-if-changed=src/deep-decoder-support.h')){
 $marker='println!("cargo:rerun-if-changed=src/bounded_readback.h");'
 if(-not $build.Contains($marker)){throw 'SDK build marker changed'}
 [IO.File]::WriteAllText("$target/build.rs",$build.Replace($marker,$marker+"`n"+'            println!("cargo:rerun-if-changed=src/deep-decoder-support.h");'))
}
$env:PATH="$root/.local/gstreamer/bin;$(Split-Path -Parent $node);$env:PATH"
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64="$root/.local/gstreamer"
$env:PKG_CONFIG_PATH="$root/.local/gstreamer/lib/pkgconfig"
$scratch=Get-ChildItem "$root/target/release/build" -Directory -Filter 'scratch-*'|Where-Object {Test-Path "$($_.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release/include"}|Select-Object -First 1
$env:LK_CUSTOM_WEBRTC="$($scratch.FullName)/out/livekit_webrtc/livekit/win-x64-release-webrtc-a545de4/win-x64-release"
cargo fmt --all
if($LASTEXITCODE -ne 0){throw 'Rust formatting failed'}
$override='patch.crates-io.webrtc-sys.path="'+$target.Replace('\','/')+'"'
$configArgs=@('--config',$override)
if($ObserveGl){
 & $node "$root/scripts/prepare-deep-gl.mjs" $Label
 if($LASTEXITCODE -ne 0){throw 'Local Glow observer preparation failed'}
 $gl=[IO.Path]::GetFullPath("$root/.local/experiments/deep-pass/gl-$Label/egui_glow-0.37.0").Replace('\','/')
 $configArgs+=@('--config',('patch.crates-io.egui_glow.path="'+$gl+'"'))
 $lockPath=Join-Path $root 'Cargo.lock'
 $originalLock=[IO.File]::ReadAllBytes($lockPath)
 [IO.File]::WriteAllBytes("$root/.local/experiments/deep-pass/gl-$Label/Cargo.original.lock",$originalLock)
 try {
  cargo rustc --offline --release -p freedomstream-publisher --example room-benchmark --features performance-experiments @configArgs -- --cfg freedomstream_deep_gl
  $buildExit=$LASTEXITCODE
  Copy-Item -LiteralPath $lockPath -Destination "$root/.local/experiments/deep-pass/gl-$Label/Cargo.observer.lock"
 }finally{[IO.File]::WriteAllBytes($lockPath,$originalLock)}
 if($buildExit -ne 0){throw 'Instrumented release build failed; original lock restored'}
 & $node "$root/scripts/check-deep-gl-lock.mjs" $Label
 if($LASTEXITCODE -ne 0){throw 'Observer changed the resolved dependency graph'}
}else{
 cargo build --locked --release -p freedomstream-publisher --example room-benchmark --features performance-experiments @configArgs
}
if($LASTEXITCODE -ne 0){throw 'Experimental application build failed'}
$exe=".local/experiments/deep-pass/$Label-room.exe"
Copy-Item -LiteralPath "$root/target/release/examples/room-benchmark.exe" -Destination "$root/$exe"
& $node "$root/scripts/record-deep-build.mjs" $Label $exe
if($LASTEXITCODE -ne 0){throw 'Build source identity capture failed'}
& $node "$root/scripts/setup-native-decoder.mjs"
if($LASTEXITCODE -ne 0){throw 'Original SDK changed during isolated build'}
