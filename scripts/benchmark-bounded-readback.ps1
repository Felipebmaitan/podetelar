param([ValidateRange(3,7)][int]$Pairs=3, [ValidateSet('bounded','bounded-v2')][string]$Candidate='bounded')
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$run=Join-Path $root ('.local/experiments/readback-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run|Out-Null
$rows=[Collections.Generic.List[object]]::new()
foreach($size in @('1080p','4k')) { foreach($contexts in @(1,4)) { foreach($visibility in @('visible','hidden')) { for($pair=1;$pair -le $Pairs;$pair++) {
 $checksums=@{}
 foreach($variant in $(if($pair%2){@('original',$Candidate)}else{@($Candidate,'original')})) {
  $exe=Join-Path $root ".local/experiments/readback-$variant/decoder-resource.exe"
  $report=Join-Path $run "$size-$contexts-$visibility-$pair-$variant.json"
  $mode=if($visibility -eq 'hidden'){'deferred-hidden'}else{'deferred'}
  $info=[Diagnostics.ProcessStartInfo]::new($exe)
  $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
  foreach($arg in @($mode,"$root/.local/experiments/fixtures/pattern-${size}60.h265",$report,"$contexts",'4')){$info.ArgumentList.Add($arg)}
  $p=[Diagnostics.Process]::Start($info);$a=$p.StandardOutput.ReadToEndAsync();$b=$p.StandardError.ReadToEndAsync()
  $clock=[Diagnostics.Stopwatch]::StartNew();$private=0L;$working=0L;$threads=0
  try {
   while(-not $p.WaitForExit(50)) {
    if($clock.Elapsed.TotalSeconds -gt 45){throw 'Owned bounded readback experiment timed out.'}
    $p.Refresh();$private=[Math]::Max($private,$p.PrivateMemorySize64);$working=[Math]::Max($working,$p.WorkingSet64);$threads=[Math]::Max($threads,$p.Threads.Count)
   }
   $null=$a.GetAwaiter().GetResult();$null=$b.GetAwaiter().GetResult()
   $r=Get-Content -Raw -LiteralPath $report|ConvertFrom-Json
   $expected=if($visibility -eq 'hidden'){0}else{300*$contexts}
   if($p.ExitCode -ne 0 -or -not $r.completed -or $r.decodeErrors -ne 0 -or $r.decoded -ne 300*$contexts -or $r.sampledPixelFrames -ne 6*$contexts -or $r.deferredMaterializationsBeforeHash -ne $expected -or $r.failedMaterializations -ne 0){throw 'Bounded readback failed frame/pixel/materialization validation.'}
   $checksums[$variant]=$r.visiblePlaneChecksumHex
   $rows.Add(@{size=$size;contexts=$contexts;visibility=$visibility;pair=$pair;variant=$variant;executableSha256=(Get-FileHash $exe).Hash;sampledPeakPrivateBytes=$private;sampledPeakWorkingSetBytes=$working;peakThreads=$threads;result=$r})
  } finally {if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
 }
 if($checksums.original -ne $checksums[$Candidate]){throw 'Bounded readback changed sampled whole-plane pixels.'}
 Write-Output "Readback $size $contexts contexts $visibility pair $pair preserved counts and sampled whole planes."
} } } }
@{schema=1;runs=$rows.ToArray();sources=@{original=(Get-Content "$root/.local/experiments/readback-original/source.json" -Raw|ConvertFrom-Json);bounded=(Get-Content "$root/.local/experiments/readback-$Candidate/source.json" -Raw|ConvertFrom-Json)};limits='Same deferred interface and original Decoder class in both variants. Only readback staging ownership differs. CPU/wall exclude full-plane hashing of six retained validation frames per context; visible variants have the same validation ownership. Hidden frames retain GPU samples until post-timing validation in both variants. These are isolated peaks, not ordinary app RAM.'}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $run 'summary.json') -Encoding utf8
Write-Output "Saved $run/summary.json"
