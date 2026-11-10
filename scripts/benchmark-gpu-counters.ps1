param([ValidateRange(3,5)][int]$Pairs=3)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot;$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$run=Join-Path $root ('.local/experiments/gpu-counters-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $run|Out-Null
$rows=[Collections.Generic.List[object]]::new()
foreach($size in @('1080p','4k')){for($pair=1;$pair -le $Pairs;$pair++){
 foreach($mode in $(if($pair%2){@('readback','gpu')}else{@('gpu','readback')})){
  $width=if($size -eq '4k'){3840}else{1920};$height=$width*9/16
  $report=Join-Path $run "$size-$pair-$mode.json";$osReport=Join-Path $run "$size-$pair-$mode-process.json"
  $info=[Diagnostics.ProcessStartInfo]::new("$root/target/release/examples/gpu-present-minimal.exe");$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
  foreach($arg in @($mode,'h265',"$root/.local/experiments/fixtures/pattern-${size}60.h265","$width","$height",'60','300',$report,'960','540')){$info.ArgumentList.Add($arg)}
  $p=[Diagnostics.Process]::Start($info);$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync()
  $samplerInfo=[Diagnostics.ProcessStartInfo]::new('pwsh.exe');$samplerInfo.UseShellExecute=$false;$samplerInfo.CreateNoWindow=$true;$samplerInfo.RedirectStandardOutput=$true;$samplerInfo.RedirectStandardError=$true
  foreach($arg in @('-NoProfile','-NonInteractive','-WindowStyle','Hidden','-File',"$root/scripts/sample-native-process.ps1",'-ProcessId',"$($p.Id)",'-Output',$osReport,'-Seconds','20')){$samplerInfo.ArgumentList.Add($arg)}
  $sampler=[Diagnostics.Process]::Start($samplerInfo);$a=$sampler.StandardOutput.ReadToEndAsync();$b=$sampler.StandardError.ReadToEndAsync()
  try{
   if(-not $p.WaitForExit(15000)){$p.Kill();throw 'GPU counter presentation timed out.'}
   $null=$out.GetAwaiter().GetResult();$null=$err.GetAwaiter().GetResult()
   if(-not $sampler.WaitForExit(10000)){$sampler.Kill();throw 'Owned GPU sampler timed out.'};$null=$a.GetAwaiter().GetResult();$null=$b.GetAwaiter().GetResult()
   $r=Get-Content -Raw -LiteralPath $report|ConvertFrom-Json
   if($p.ExitCode -ne 0 -or $r.status -ne 'completed'){throw 'GPU counter comparison did not preserve frame/pacing validation.'}
   $rows.Add(@{size=$size;pair=$pair;mode=$mode;pipeline=$r;process=(Get-Content -Raw -LiteralPath $osReport|ConvertFrom-Json)})
   Write-Output "GPU counters $size pair $pair $mode validated all 300 frames."
  }finally{foreach($owned in @($p,$sampler)){if(-not $owned.HasExited){$owned.Kill();$owned.WaitForExit()};$owned.Dispose()}}
 }
}}
$output=Join-Path $run 'summary.json';@{schema=1;runs=$rows.ToArray();exeSha256=(Get-FileHash "$root/target/release/examples/gpu-present-minimal.exe").Hash;limits='Actual per-process GPU engine and driver memory counters alongside native pipeline validation. Separate engines are not added into one utilization value. Isolated pipeline, no SDK/egui app integration or game claim.'}|ConvertTo-Json -Depth 14|Set-Content -LiteralPath $output -Encoding utf8
Write-Output "Saved $output"
