param([Parameter(Mandatory=$true)][string]$Cases,[ValidateRange(1,5)][int]$Pairs=3)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot;$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$casesPath=(Resolve-Path -LiteralPath $Cases).Path;$workloads=Get-Content -Raw -LiteralPath $casesPath|ConvertFrom-Json
$run=Join-Path $root ('.local/experiments/pipelines/run-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $run|Out-Null
$rows=[Collections.Generic.List[object]]::new()
for($pair=1;$pair -le $Pairs;$pair++){
 $ordered=@($workloads);if($pair%2 -eq 0){[array]::Reverse($ordered)}
 foreach($case in $ordered){
  if($case.label -notmatch '^[a-z0-9-]{1,80}$'){throw 'Invalid synthetic case label.'}
  $inputPath=Join-Path $run "$($case.label)-$pair-input.json";$report=Join-Path $run "$($case.label)-$pair.json"
  $case.request|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $inputPath -Encoding utf8NoBOM
  $info=[Diagnostics.ProcessStartInfo]::new("$root/target/release/examples/pipeline-resource.exe");$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
  $info.ArgumentList.Add($inputPath);$info.ArgumentList.Add($report)
  $p=[Diagnostics.Process]::Start($info);$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync();$clock=[Diagnostics.Stopwatch]::StartNew();$private=0L;$working=0L;$threads=0;$handles=0
  try {
   while(-not $p.WaitForExit(10)){
    if($clock.Elapsed.TotalSeconds -gt 80){throw 'Owned synthetic pipeline timed out.'}
    $p.Refresh();$private=[Math]::Max($private,$p.PrivateMemorySize64);$working=[Math]::Max($working,$p.WorkingSet64);$threads=[Math]::Max($threads,$p.Threads.Count);$handles=[Math]::Max($handles,$p.HandleCount)
   }
   $null=$stdout.GetAwaiter().GetResult();$null=$stderr.GetAwaiter().GetResult();$r=Get-Content -Raw -LiteralPath $report|ConvertFrom-Json
   $rows.Add(@{pair=$pair;label=$case.label;valid=($p.ExitCode -eq 0 -and $r.completed -and $r.valid);cpuMs=$p.TotalProcessorTime.TotalMilliseconds;sampledPeakPrivateBytes=$private;sampledPeakWorkingSetBytes=$working;peakThreads=$threads;peakHandles=$handles;result=$r})
   Write-Output "Synthetic $($case.label) pair $pair completed=$($r.completed) valid=$($r.valid)."
  }finally{if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
 }
}
$output=Join-Path $run 'summary.json';@{schema=1;runs=$rows.ToArray();caseSha256=(Get-FileHash -LiteralPath $casesPath).Hash;exeSha256=(Get-FileHash "$root/target/release/examples/pipeline-resource.exe").Hash;limits='Serial synthetic pipelines. Process CPU includes startup; memory/thread/handle peaks sampled every 10ms. Invalid runs remain invalid. PCM values and logical buffers do not measure physical audio latency.'}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $output -Encoding utf8
Write-Output "Saved $output"
