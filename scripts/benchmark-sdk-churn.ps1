param([ValidateRange(3,7)][int]$Pairs=5)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$env:PATH="$root/.local/gstreamer/bin;$env:PATH"
$run=Join-Path $root ('.local/experiments/sdk-churn-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run | Out-Null
$rows=[Collections.Generic.List[object]]::new()
for($pair=1;$pair -le $Pairs;$pair++){
  foreach($mode in $(if($pair%2){@('baseline','pruned')}else{@('pruned','baseline')})){
    $exe=Join-Path $root $(if($mode -eq 'baseline'){'.local/experiments/pool/room-benchmark.exe'}else{'.local/experiments/deferred-i420-pool/room-benchmark.exe'})
    $report=Join-Path $run "$pair-$mode.json"
    $info=[Diagnostics.ProcessStartInfo]::new($exe);$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $info.ArgumentList.Add('--sdk-soak');$info.ArgumentList.Add($report)
    $p=[Diagnostics.Process]::Start($info);$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync()
    $clock=[Diagnostics.Stopwatch]::StartNew();$samples=[Collections.Generic.List[object]]::new()
    try {
      while(-not $p.WaitForExit(50)){
        if($clock.Elapsed.TotalSeconds -gt 20){throw 'Owned SDK churn experiment timed out.'}
        $p.Refresh();$samples.Add(@{second=$clock.Elapsed.TotalSeconds;cpuMs=$p.TotalProcessorTime.TotalMilliseconds;privateBytes=$p.PrivateMemorySize64;workingSetBytes=$p.WorkingSet64;threads=$p.Threads.Count;handles=$p.HandleCount})
      }
      $null=$out.GetAwaiter().GetResult();$null=$err.GetAwaiter().GetResult()
      $r=Get-Content -Raw $report|ConvertFrom-Json
      if($p.ExitCode -ne 0 -or $r.rounds.Count -ne 20 -or $r.rounds[-1].tracksCreatedAndDropped -ne 10000){throw 'Actual SDK track churn failed.'}
      $rows.Add(@{pair=$pair;mode=$mode;exeSha256=(Get-FileHash $exe).Hash;result=$r;processSamples=$samples.ToArray()})
    } finally {if(-not $p.HasExited){$p.Kill();$p.WaitForExit()};$p.Dispose()}
    Write-Output "Actual SDK churn pair $pair ${mode}: 10,000 tracks created and dropped."
  }
}
@{schema=1;runs=$rows.ToArray();limits='Actual SDK factory and 10,000 sequential track creations/drops per child. Decoder/image optimizations are inactive because no frames are submitted. No log text or process identities exported. This stress result does not describe normal per-frame savings or a frame/decoder leak.'}|ConvertTo-Json -Depth 10|Set-Content "$run/summary.json" -Encoding utf8
Write-Output "Saved $run/summary.json"
