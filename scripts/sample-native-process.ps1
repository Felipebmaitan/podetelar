param(
    [Parameter(Mandatory=$true)][int]$ProcessId,
    [Parameter(Mandatory=$true)][string]$Output,
    [ValidateRange(1,300)][int]$Seconds=120,
    [ValidateRange(1,20)][int]$RestoreAfterSeconds=5
)
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class BenchmarkWindow {
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
 [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr window, int command);
}
'@
$outputPath=[IO.Path]::GetFullPath($Output)
if(Test-Path -LiteralPath $outputPath){throw 'Choose a new measurement file.'}
$clock=[Diagnostics.Stopwatch]::StartNew()
$startedAt=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$samples=[Collections.Generic.List[object]]::new()
$counterFailures=0
$minimizedAt=$null
while($clock.Elapsed.TotalSeconds -lt $Seconds){
    $target=Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if(-not $target){break}
    $engines=@{}; $memory=@{}
    try{
        $gpu=Get-Counter -Counter @("\GPU Engine(pid_${ProcessId}_*)\Utilization Percentage","\GPU Process Memory(pid_${ProcessId}_*)\Dedicated Usage","\GPU Process Memory(pid_${ProcessId}_*)\Shared Usage","\GPU Process Memory(pid_${ProcessId}_*)\Total Committed") -SampleInterval 1 -MaxSamples 1 -ErrorAction Stop
        foreach($counter in $gpu.CounterSamples){
            if($counter.InstanceName -notmatch "^pid_${ProcessId}_"){continue}
            if($counter.Path -match 'gpu engine'){
                $kind=($counter.InstanceName -split '_engtype_')[-1]
                if(-not $engines.ContainsKey($kind)){$engines[$kind]=0.0}
                $engines[$kind]+=$counter.CookedValue
            }else{
                $kind=($counter.Path -split '\\')[-1]
                if(-not $memory.ContainsKey($kind)){$memory[$kind]=0.0}
                $memory[$kind]+=$counter.CookedValue
            }
        }
    }catch{$counterFailures++}
    try{
        $target.Refresh()
        $window=$target.MainWindowHandle
        if($window -ne [IntPtr]::Zero -and [BenchmarkWindow]::IsIconic($window)){
            if($null -eq $minimizedAt){$minimizedAt=$clock.Elapsed.TotalSeconds}
            if($clock.Elapsed.TotalSeconds-$minimizedAt -ge $RestoreAfterSeconds){$null=[BenchmarkWindow]::ShowWindowAsync($window,9)}
        }else{$minimizedAt=$null}
        $samples.Add(@{second=$clock.Elapsed.TotalSeconds;cpuMs=$target.TotalProcessorTime.TotalMilliseconds;privateBytes=$target.PrivateMemorySize64;workingSetBytes=$target.WorkingSet64;threads=$target.Threads.Count;handles=$target.HandleCount;gpuEnginesPercent=$engines;gpuMemoryBytes=$memory})
    }catch{break}
    $target.Dispose()
    [Threading.Thread]::Sleep(100)
}
@{schema=1;processId=$ProcessId;startedAt=$startedAt;samplerCpuMs=(Get-Process -Id $PID).TotalProcessorTime.TotalMilliseconds;counterFailures=$counterFailures;samples=$samples.ToArray();limits='PDH samples per owned process. Engine sums are reported by engine type, not a single GPU percentage. Memory counters include driver accounting; not a heap or texture-size measurement.'}|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $outputPath -Encoding utf8
