param([ValidateSet('device','readback','qualification','lifecycle','pool-trace')][string]$Family='device',[ValidatePattern('^[a-z0-9-]{0,20}$')][string]$RunLabel='',[switch]$FlushMap)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$out=Join-Path $root ".local/experiments/deep-pass/native-$Family$RunLabel"
if(Test-Path -LiteralPath $out){throw 'Existing measurements are immutable; choose a new output family'}
New-Item -ItemType Directory -Path $out|Out-Null
$exe=Join-Path $root '.local/experiments/deep-pass/decoder/deep-decoder.exe'
$rows=[Collections.Generic.List[object]]::new()
$cases=[Collections.Generic.List[object]]::new()
if($Family -in @('device','readback')){
 foreach($size in @('1080p','4k')){foreach($contexts in @(1,4)){foreach($visibility in @('visible','hidden')){
  $pairs=if($contexts -eq 4 -and $visibility -eq 'visible'){3}else{1}
  for($pair=1;$pair -le $pairs;$pair++){foreach($variant in $(if($pair%2){@('before','after')}else{@('after','before')})){
   $cases.Add(@{size=$size;contexts=$contexts;visibility=$visibility;pair=$pair;variant=$variant;codec='h265';cycles=1})
  }}
 }}}
}elseif($Family -eq 'qualification'){
 foreach($codec in @('h264','h265','av1')){foreach($variant in @('baseline','device','readback')){
  $cases.Add(@{size='1080p';contexts=1;visibility='visible';pair=1;variant=$variant;codec=$codec;cycles=1})
 }}
 foreach($variant in @('baseline','device','readback')){$cases.Add(@{size='1080p';contexts=4;visibility='visible';pair=1;variant=$variant;codec='h265';cycles=1})}
}elseif($Family -eq 'lifecycle'){
 foreach($variant in @('repeat-a','repeat-b')){$cases.Add(@{size='1080p';contexts=4;visibility='visible';pair=1;variant=$variant;codec='h265';cycles=10})}
}else{
 foreach($visibility in @('visible','hidden')){$cases.Add(@{size='1080p';contexts=4;visibility=$visibility;pair=1;variant='baseline';codec='h265';cycles=1})}
}
foreach($case in $cases){
 $stem="$($case.size)-$($case.contexts)-$($case.visibility)-$($case.codec)-$($case.pair)-$($case.variant)"
 $report=Join-Path $out "$stem.json"
 $info=[Diagnostics.ProcessStartInfo]::new($exe)
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 foreach($arg in @("$root/.local/experiments/fixtures/pattern-$($case.size)60.$($case.codec)",$report,"$($case.contexts)","$($case.cycles)",$case.visibility,$case.codec)){$info.ArgumentList.Add($arg)}
 $info.Environment['FREEDOMSTREAM_DEEP_SHARED_DEVICE']=if(($Family -eq 'device' -and $case.variant -eq 'after')-or $case.variant -eq 'device'){'1'}else{'0'}
 $info.Environment['FREEDOMSTREAM_DEEP_RELAXED_MAP']=if(($Family -eq 'readback' -and $case.variant -eq 'after')-or $case.variant -eq 'readback'){'1'}else{'0'}
 $info.Environment['FREEDOMSTREAM_DEEP_FLUSH_MAP']=if($FlushMap -and (($Family -eq 'readback' -and $case.variant -eq 'after')-or $case.variant -eq 'readback')){'1'}else{'0'}
 $info.Environment['FREEDOMSTREAM_DEEP_HASH_ALL']=if($Family -eq 'qualification'){'1'}else{'0'}
 $info.Environment['FREEDOMSTREAM_DEEP_POOL_TRACE']=if($Family -eq 'pool-trace'){'1'}else{'0'}
 $p=[Diagnostics.Process]::Start($info);$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
 $clock=[Diagnostics.Stopwatch]::StartNew();$private=0L;$working=0L
 try {
  while(-not $p.WaitForExit(50)){
   if($clock.Elapsed.TotalSeconds -gt 15*$case.cycles+20){throw 'Owned decoder case timed out'}
   $p.Refresh();$private=[Math]::Max($private,$p.PrivateMemorySize64);$working=[Math]::Max($working,$p.WorkingSet64)
  }
  $null=$stdout.GetAwaiter().GetResult();$null=$stderr.GetAwaiter().GetResult()
  if(-not (Test-Path -LiteralPath $report)){throw 'Decoder case produced no report'}
  $r=Get-Content -LiteralPath $report -Raw|ConvertFrom-Json
  $valid=$p.ExitCode -eq 0 -and $r.completed -and @($r.runs|Where-Object {$_.decoded -ne 300*$case.contexts -or $_.errors -ne 0 -or $_.afterTimed.failures -ne 0}).Count -eq 0
  $rows.Add(@{case=$case;valid=$valid;exitCode=$p.ExitCode;sampledPeakPrivateBytes=$private;sampledPeakWorkingSetBytes=$working;result=$r;file=$report})
  @{schema=1;family=$Family;exeSha256=(Get-FileHash $exe).Hash;runs=$rows.ToArray()}|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $out 'summary.json') -Encoding utf8
  Write-Output (@{case=$stem;valid=$valid;cycles=$r.runs.Count;devicesCreated=$r.runs[0].afterTimed.devicesCreated;mapBusy=$r.runs[0].afterTimed.mapBusy}|ConvertTo-Json -Compress)
  if(-not $valid){throw 'Decoder count/error assertion failed; evidence retained'}
 }finally{if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
}
