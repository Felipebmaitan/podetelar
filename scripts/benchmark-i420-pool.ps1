param([ValidateRange(3,7)][int]$Pairs=5)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$run=Join-Path $root ('.local/experiments/i420-pool-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run | Out-Null
$exe=Join-Path $root '.local/experiments/decoder/decoder-resource.exe'
function Invoke-Owned([string[]]$Arguments) {
  $info=[Diagnostics.ProcessStartInfo]::new($exe)
  $info.UseShellExecute=$false;$info.CreateNoWindow=$true
  $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
  foreach($arg in $Arguments){$info.ArgumentList.Add($arg)}
  $p=[Diagnostics.Process]::Start($info)
  $out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync()
  $private=0L;$working=0L;$clock=[Diagnostics.Stopwatch]::StartNew()
  try {
    while(-not $p.WaitForExit(10)) {
      if($clock.Elapsed.TotalSeconds -gt 20){throw 'Owned I420 pool experiment timed out.'}
      $p.Refresh();$private=[Math]::Max($private,$p.PrivateMemorySize64)
      $working=[Math]::Max($working,$p.WorkingSet64)
    }
    $null=$out.GetAwaiter().GetResult();$null=$err.GetAwaiter().GetResult()
    if($p.ExitCode -ne 0){throw 'Pinned I420 pool probe failed.'}
    return @{sampledPeakPrivateBytes=$private;sampledPeakWorkingSetBytes=$working}
  } finally {if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
}
$null=Invoke-Owned @('--check-pool',"$run/check.json")
$check=Get-Content -Raw "$run/check.json" | ConvertFrom-Json
if(-not $check.valid){throw 'Pool ownership/bounds check failed.'}
$rows=[Collections.Generic.List[object]]::new()
foreach($width in @(1920,3840)){foreach($contexts in @(1,4)){for($pair=1;$pair -le $Pairs;$pair++){
  $checksums=@{}
  foreach($mode in $(if($pair%2){@('i420-fresh','i420-pooled')}else{@('i420-pooled','i420-fresh')})){
    $report=Join-Path $run "$width-$contexts-$pair-$mode.json"
    $os=Invoke-Owned @($mode,$report,"$width","$contexts")
    $r=Get-Content -Raw $report | ConvertFrom-Json
    if(-not $r.completed -or $r.errors){throw 'I420 pool conversion failed.'}
    $checksums[$mode]=$r.wholeOutputChecksumHex
    $rows.Add(@{pair=$pair;mode=$mode;process=$os;result=$r})
  }
  if($checksums['i420-fresh'] -ne $checksums['i420-pooled']){throw 'I420 pool output pixels changed.'}
}}}
@{schema=1;ownership=$check;runs=$rows.ToArray();executableSha256=(Get-FileHash $exe).Hash;limits='Actual pinned WebRTC bounded pool, full conversion and whole-output checks. Not integrated into the application.'}|ConvertTo-Json -Depth 9 | Set-Content "$run/summary.json" -Encoding utf8
Write-Output "Saved $run/summary.json"
