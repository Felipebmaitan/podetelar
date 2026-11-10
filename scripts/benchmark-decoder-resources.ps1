param([ValidateSet('1080p','4k')][string]$Size='1080p',[switch]$SoftwareOnly)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$run=Join-Path $root ('.local/experiments/decoder/run-'+[guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $run|Out-Null
$rows=[Collections.Generic.List[object]]::new()
function Measure-Decoder([string]$mode,[int]$contexts,[int]$cores,[int]$pair){
 $report=Join-Path $run "$mode-$contexts-$cores-$pair.json"
 $fixture=Join-Path $root $(if($mode -eq 'av1'){'.local/experiments/fixtures/pattern-'+$Size+'60.av1'}else{'.local/experiments/fixtures/pattern-'+$Size+'60.h265'})
 $info=[Diagnostics.ProcessStartInfo]::new("$root/.local/experiments/decoder/decoder-resource.exe");$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 foreach($arg in @($mode,$fixture,$report,"$contexts","$cores")){$info.ArgumentList.Add($arg)}
 $p=[Diagnostics.Process]::Start($info);$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync();$clock=[Diagnostics.Stopwatch]::StartNew();$peakPrivate=0L;$peakWorking=0L;$peakThreads=0
 try {
  while(-not $p.WaitForExit(50)){if($clock.Elapsed.TotalSeconds -gt 20){throw 'Owned decoder probe timed out.'};$p.Refresh();$peakPrivate=[Math]::Max($peakPrivate,$p.PrivateMemorySize64);$peakWorking=[Math]::Max($peakWorking,$p.WorkingSet64);$peakThreads=[Math]::Max($peakThreads,$p.Threads.Count)}
  $null=$stdout.GetAwaiter().GetResult();$null=$stderr.GetAwaiter().GetResult();$r=Get-Content -Raw -LiteralPath $report|ConvertFrom-Json
  $valid=$p.ExitCode -eq 0 -and $r.completed -and $r.decodeErrors -eq 0 -and ($mode -ne 'av1' -or $r.decoded -eq 300*$contexts)
  $rows.Add(@{mode=$mode;contexts=$contexts;cores=$cores;pair=$pair;valid=$valid;peakPrivateBytes=$peakPrivate;peakWorkingSetBytes=$peakWorking;peakThreads=$peakThreads;result=$r})
  Write-Output "Decoder $mode contexts $contexts cores $cores pair $pair valid=$valid."
 }finally{if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
}
for($pair=1;$pair -le 3;$pair++){
 if(-not $SoftwareOnly){foreach($mode in $(if($pair%2){@('baseline','bounded')}else{@('bounded','baseline')})){Measure-Decoder $mode 1 4 $pair}}
 foreach($contexts in @(1,4)) {foreach($cores in $(if($pair%2){@(1,2,4,8,16)}else{@(16,8,4,2,1)})){Measure-Decoder 'av1' $contexts $cores $pair}}
}
$output=Join-Path $run 'summary.json';@{schema=1;size=$Size;runs=$rows.ToArray();source=(Get-Content -Raw "$root/.local/experiments/decoder/source.json"|ConvertFrom-Json);fixtureSha256=@{hevc=(Get-FileHash "$root/.local/experiments/fixtures/pattern-${Size}60.h265").Hash;av1=(Get-FileHash "$root/.local/experiments/fixtures/pattern-${Size}60.av1").Hash}}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $output -Encoding utf8
Write-Output "Saved $output"
