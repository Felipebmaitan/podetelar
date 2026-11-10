param([Parameter(Mandatory=$true)][string]$Before,[Parameter(Mandatory=$true)][string]$After,[Parameter(Mandatory=$true)][string]$Payload,[ValidateRange(3,7)][int]$Pairs=5)
$ErrorActionPreference='Stop'
$run=Join-Path (Get-Location) ('.local/experiments/portable-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run|Out-Null
$payloadPath=(Resolve-Path -LiteralPath $Payload).Path
function Package([string]$wrapper,[string]$destination){
 $target=[IO.File]::Open($destination,[IO.FileMode]::CreateNew)
 try {
  foreach($path in @($wrapper,$payloadPath)){$source=[IO.File]::OpenRead($path);try{$source.CopyTo($target,65536)}finally{$source.Dispose()}}
  $source=[IO.File]::OpenRead($payloadPath);$sha=[Security.Cryptography.SHA256]::Create()
  try{$digest=$sha.ComputeHash($source)}finally{$source.Dispose();$sha.Dispose()}
  $target.Write([BitConverter]::GetBytes([uint64](Get-Item -LiteralPath $payloadPath).Length));$target.Write($digest);$target.Write([Text.Encoding]::ASCII.GetBytes('FREEDOMSTREAM01!'))
 }finally{$target.Dispose()}
}
$packages=@{}
foreach($variant in @('before','after')){
 $wrapper=(Resolve-Path -LiteralPath $(if($variant -eq 'before'){$Before}else{$After})).Path
 $packages[$variant]=Join-Path $run "$variant.exe"
 Package $wrapper $packages[$variant]
}
function Start-Owned([string]$exe,[string]$data){
 $info=[Diagnostics.ProcessStartInfo]::new($exe);$info.UseShellExecute=$false;$info.CreateNoWindow=$true
 $info.ArgumentList.Add('--watch-fixture');$info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $info.Environment['LOCALAPPDATA']=$data;$info.Environment['PATH']="$env:WINDIR/System32;$env:WINDIR"
 $process=[Diagnostics.Process]::Start($info)
 @{process=$process;stdout=$process.StandardOutput.ReadToEndAsync();stderr=$process.StandardError.ReadToEndAsync();clock=[Diagnostics.Stopwatch]::StartNew();nativeId=$null;peakSampledPrivate=0L;peakSampledWorking=0L;finished=$false}
}
function Finish-Owned($job){
 $p=$job.process
 try {
  while(-not $job.nativeId){
   if($p.HasExited -or $job.clock.Elapsed.TotalSeconds -gt 20){throw 'Owned portable startup failed.'}
   $p.Refresh();$job.peakSampledPrivate=[Math]::Max($job.peakSampledPrivate,$p.PrivateMemorySize64);$job.peakSampledWorking=[Math]::Max($job.peakSampledWorking,$p.WorkingSet64)
   $child=Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)"|Where-Object Name -eq 'freedomstream-publisher.exe'|Select-Object -First 1
   if($child){$job.nativeId=[int]$child.ProcessId}
  }
  $p.Refresh()
  @{startupToObservedChildMs=$job.clock.Elapsed.TotalMilliseconds;wrapperCpuMs=$p.TotalProcessorTime.TotalMilliseconds;peakCommitBytes=$p.PeakPagedMemorySize64;peakWorkingSetBytes=$p.PeakWorkingSet64;sampledPeakPrivateBytes=$job.peakSampledPrivate;waitingPrivateBytes=$p.PrivateMemorySize64;waitingWorkingSetBytes=$p.WorkingSet64}
 }finally{
  if(-not $p.HasExited){$p.Kill();$p.WaitForExit()}
  if($job.nativeId){for($n=0;$n -lt 30 -and (Get-Process -Id $job.nativeId -ErrorAction SilentlyContinue);$n++){[Threading.Thread]::Sleep(100)};if(Get-Process -Id $job.nativeId -ErrorAction SilentlyContinue){throw 'Lifetime job failed to stop owned child.'}}
  $null=$job.stdout.GetAwaiter().GetResult();$null=$job.stderr.GetAwaiter().GetResult();$p.Dispose();$job.finished=$true
 }
}
$rows=[Collections.Generic.List[object]]::new()
for($pair=1;$pair -le $Pairs;$pair++){
 foreach($variant in $(if($pair%2){@('before','after')}else{@('after','before')})){
  $data=Join-Path $run "$pair-$variant";New-Item -ItemType Directory -Path $data|Out-Null
  foreach($state in @('cold','warm')){$job=Start-Owned $packages[$variant] $data;$rows.Add(@{pair=$pair;variant=$variant;state=$state;result=(Finish-Owned $job)})}
  $concurrent=Join-Path $run "$pair-$variant-concurrent";New-Item -ItemType Directory -Path $concurrent|Out-Null
  $first=Start-Owned $packages[$variant] $concurrent;$second=Start-Owned $packages[$variant] $concurrent
  try{$a=Finish-Owned $first;$b=Finish-Owned $second;$rows.Add(@{pair=$pair;variant=$variant;state='concurrent-cold';result=@($a,$b)})}finally{foreach($job in @($first,$second)){if(-not $job.finished -and -not $job.process.HasExited){$job.process.Kill()}}}
 }
 Write-Output "Portable startup pair $pair completed; owned children cleaned up."
}
$output=Join-Path $run 'result.json'
@{schema=1;rows=$rows.ToArray();beforeSha256=(Get-FileHash -LiteralPath $Before).Hash;afterSha256=(Get-FileHash -LiteralPath $After).Hash;payloadSha256=(Get-FileHash -LiteralPath $Payload).Hash;payloadBytes=(Get-Item -LiteralPath $Payload).Length;limits='Actual identical runtime payload and real child held at stdin, no room/capture. Peak commit/working-set are Windows process lifetime peaks; waiting/private is a separate sample. Startup observation includes CIM polling overhead. Concurrent results are individual wrappers, not deduplicated process-tree RAM. Warm cache still hashes every byte.'}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $output -Encoding utf8
Write-Output "Saved $output"
