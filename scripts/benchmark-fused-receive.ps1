param([ValidateRange(3,7)][int]$Pairs=5,[switch]$Deferred)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot;$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$prefix=if($Deferred){'deferred-'}else{'fused-'}
$run=Join-Path $root ('.local/experiments/decoder/'+$prefix+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $run|Out-Null
$rows=[Collections.Generic.List[object]]::new()
foreach($size in @('1080p','4k')){foreach($contexts in @(1,4)){foreach($visibility in $(if($Deferred){@('visible','hidden')}else{@('visible')})){for($pair=1;$pair -le $Pairs;$pair++){
 $checksums=@{}
 $before=if($visibility -eq 'hidden'){'direct-hidden'}else{'direct'}
 $after=if($Deferred){if($visibility -eq 'hidden'){'deferred-hidden'}else{'deferred'}}else{'fused'}
 foreach($mode in $(if($pair%2){@($before,$after)}else{@($after,$before)})){
  $report=Join-Path $run "$size-$contexts-$visibility-$pair-$mode.json"
  $info=[Diagnostics.ProcessStartInfo]::new("$root/.local/experiments/decoder/decoder-resource.exe");$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
  foreach($arg in @($mode,"$root/.local/experiments/fixtures/pattern-${size}60.h265",$report,"$contexts",'4')){$info.ArgumentList.Add($arg)}
  $p=[Diagnostics.Process]::Start($info);$a=$p.StandardOutput.ReadToEndAsync();$b=$p.StandardError.ReadToEndAsync();$clock=[Diagnostics.Stopwatch]::StartNew();$private=0L;$working=0L;$threads=0
  try{
   while(-not $p.WaitForExit(50)){if($clock.Elapsed.TotalSeconds -gt 25){throw 'Owned fused decoder experiment timed out.'};$p.Refresh();$private=[Math]::Max($private,$p.PrivateMemorySize64);$working=[Math]::Max($working,$p.WorkingSet64);$threads=[Math]::Max($threads,$p.Threads.Count)}
   $null=$a.GetAwaiter().GetResult();$null=$b.GetAwaiter().GetResult();$r=Get-Content -Raw -LiteralPath $report|ConvertFrom-Json
   if($p.ExitCode -ne 0 -or -not $r.completed -or $r.decodeErrors -ne 0 -or $r.decoded -ne 300*$contexts -or $r.sampledPixelFrames -ne 6*$contexts){throw 'Fused decoder comparison failed frame/codec validation.'}
   if($Deferred -and $mode -eq $after){
    $expected=if($visibility -eq 'hidden'){0}else{300*$contexts}
    if($r.deferredMaterializationsBeforeHash -ne $expected -or $r.failedMaterializations -ne 0){throw 'Deferred plane materialization behavior changed.'}
   }
   $checksums[$mode]=$r.visiblePlaneChecksumHex
   $rows.Add(@{size=$size;contexts=$contexts;visibility=$visibility;pair=$pair;mode=$mode;sampledPeakPrivateBytes=$private;sampledPeakWorkingSetBytes=$working;peakThreads=$threads;result=$r})
  }finally{if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
 }
 if($checksums[$before] -ne $checksums[$after]){throw 'Decoder changed visible I420 pixels; reject the patch.'}
 Write-Output "Receive comparison $size $contexts contexts $visibility pair $pair matched all frames and sampled whole-frame pixels."
}}}}
$output=Join-Path $run 'summary.json';@{schema=1;runs=$rows.ToArray();source=(Get-Content -Raw "$root/.local/experiments/decoder/source.json"|ConvertFrom-Json);generatedFusedAdapterSha256=(Get-FileHash "$root/.local/experiments/decoder/decoder-fused-adapter.h").Hash;limits='CPU/wall exclude complete visible-plane hashing of six retained frames per context. Validation retains CPU images for the baseline, GPU samples for deferred hidden frames, and therefore changes memory differently. Use actual app measurements for RAM comparisons. This remains CPU presentation, with GPU readback on demand; no GPU texture bridge is claimed.'}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $output -Encoding utf8
Write-Output "Saved $output"
