param(
    [Parameter(Mandatory = $true)][string]$Before,
    [Parameter(Mandatory = $true)][string]$After,
    [Parameter(Mandatory = $true)][string]$Payload,
    [Parameter(Mandatory = $true)][string]$Output,
    [ValidateRange(3, 10)][int]$Runs = 5
)
$ErrorActionPreference = 'Stop'
$beforeRoot = (Resolve-Path -LiteralPath $Before).Path
$afterRoot = (Resolve-Path -LiteralPath $After).Path
$payloadPath = (Resolve-Path -LiteralPath $Payload).Path
$outputPath = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $outputPath) { throw 'Choose a new evidence file.' }
$evidenceRoot = Split-Path -Parent $outputPath
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null
$fixtureRoot = Join-Path $evidenceRoot ('fixtures-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null

function Package-Launcher([string]$Launcher, [string]$Destination) {
    $destinationStream = [IO.File]::Open($Destination, [IO.FileMode]::CreateNew)
    try {
        foreach ($path in @($Launcher, $payloadPath)) {
            $inputStream = [IO.File]::OpenRead($path)
            try { $inputStream.CopyTo($destinationStream, 65536) } finally { $inputStream.Dispose() }
        }
        $inputStream = [IO.File]::OpenRead($payloadPath)
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $digest = $sha.ComputeHash($inputStream) } finally { $sha.Dispose(); $inputStream.Dispose() }
        $size = [BitConverter]::GetBytes([uint64](Get-Item -LiteralPath $payloadPath).Length)
        $destinationStream.Write($size, 0, $size.Length)
        $destinationStream.Write($digest, 0, $digest.Length)
        $magic = [Text.Encoding]::ASCII.GetBytes('FREEDOMSTREAM01!')
        $destinationStream.Write($magic, 0, $magic.Length)
    } finally { $destinationStream.Dispose() }
}

function Start-Hidden([string]$Executable, [string]$Arguments, [bool]$Portable = $false) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Executable
    $info.Arguments = $Arguments
    $info.WorkingDirectory = $fixtureRoot
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    if ($Portable) {
        # Isolate saved rooms and runtime cache; never join the user's room.
        $info.EnvironmentVariables['LOCALAPPDATA'] = Join-Path $fixtureRoot 'LocalAppData'
        $info.EnvironmentVariables['PATH'] = "$env:WINDIR\System32;$env:WINDIR"
    }
    $childProcess = [Diagnostics.Process]::new()
    $childProcess.StartInfo = $info
    [void]$childProcess.Start()
    return @{ Process = $childProcess; Output = $childProcess.StandardOutput.ReadToEndAsync(); Error = $childProcess.StandardError.ReadToEndAsync() }
}

function Invoke-Conversion([string]$Executable, [string]$Arguments) {
    $run = Start-Hidden $Executable $Arguments
    $childProcess = $run.Process
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $peakPrivate = [int64]0
    $peakWorking = [int64]0
    try {
        while (-not $childProcess.HasExited) {
            if ($clock.Elapsed.TotalSeconds -gt 120) { throw 'Conversion workload timed out.' }
            $childProcess.Refresh()
            $peakPrivate = [Math]::Max($peakPrivate, $childProcess.PrivateMemorySize64)
            $peakWorking = [Math]::Max($peakWorking, $childProcess.WorkingSet64)
            [Threading.Thread]::Sleep(10)
        }
        $cpuMs = $childProcess.TotalProcessorTime.TotalMilliseconds
        if ($childProcess.ExitCode -ne 0) { throw 'Conversion workload failed; inspect its bounded standalone fixture.' }
        $result = $run.Output.GetAwaiter().GetResult() | ConvertFrom-Json
        $result | Add-Member -NotePropertyName processCpuMs -NotePropertyValue $cpuMs
        $result | Add-Member -NotePropertyName sampledPeakPrivateBytes -NotePropertyValue $peakPrivate
        $result | Add-Member -NotePropertyName sampledPeakWorkingSetBytes -NotePropertyValue $peakWorking
        return $result
    } finally {
        if (-not $childProcess.HasExited) { $childProcess.Kill(); $childProcess.WaitForExit() }
        $childProcess.Dispose()
    }
}

function Invoke-Wrapper([string]$Executable) {
    # Real publisher waits for EOF on stdin before it can capture or connect.
    # Keep that pipe open. The wrapper still supervises the real child/job.
    $run = Start-Hidden $Executable '--watch-fixture' $true
    $childProcess = $run.Process
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $nativeId = $null
    $samples = [Collections.Generic.List[object]]::new()
    try {
        while (-not $nativeId) {
            if ($childProcess.HasExited -or $clock.Elapsed.TotalSeconds -gt 15) { throw 'Portable publisher did not reach its stdin wait.' }
            $native = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($childProcess.Id)" | Where-Object Name -eq 'freedomstream-publisher.exe' | Select-Object -First 1
            if ($native) { $nativeId = [int]$native.ProcessId }
            [Threading.Thread]::Sleep(100)
        }
        [Threading.Thread]::Sleep(1000)
        $cpuStart = $childProcess.TotalProcessorTime.TotalMilliseconds
        $sampleStart = $clock.Elapsed.TotalSeconds
        while ($clock.Elapsed.TotalSeconds - $sampleStart -lt 3) {
            if ($childProcess.HasExited) { throw 'Portable wrapper exited during measurement.' }
            $childProcess.Refresh()
            $samples.Add(@{ privateBytes = $childProcess.PrivateMemorySize64; workingSetBytes = $childProcess.WorkingSet64; threads = $childProcess.Threads.Count })
            [Threading.Thread]::Sleep(50)
        }
        return @{ samples = $samples.ToArray(); waitingCpuMs = $childProcess.TotalProcessorTime.TotalMilliseconds - $cpuStart }
    } finally {
        if (-not $childProcess.HasExited) { $childProcess.Kill(); $childProcess.WaitForExit() }
        $childProcess.Dispose()
        if ($nativeId) {
            # Verify job cleanup; only the child we just created may be targeted.
            for ($attempt = 0; $attempt -lt 30 -and (Get-Process -Id $nativeId -ErrorAction SilentlyContinue); $attempt++) { [Threading.Thread]::Sleep(100) }
            if (Get-Process -Id $nativeId -ErrorAction SilentlyContinue) { throw 'The portable lifetime job did not release its child.' }
        }
    }
}

$variants = [ordered]@{ before = $beforeRoot; after = $afterRoot }
$hashes = [ordered]@{}
foreach ($name in $variants.Keys) {
    $directory = $variants[$name]
    $packaged = Join-Path $fixtureRoot "$name.exe"
    Package-Launcher (Join-Path $directory 'freedomstream-portable.exe') $packaged
    $hashes[$name] = @{ launcher = (Get-FileHash -LiteralPath (Join-Path $directory 'freedomstream-portable.exe')).Hash.ToLowerInvariant(); conversion = (Get-FileHash -LiteralPath (Join-Path $directory 'frame-conversion.exe')).Hash.ToLowerInvariant() }
    $probe = Start-Hidden $packaged '--probe' $true
    try {
        if (-not $probe.Process.WaitForExit(90000) -or $probe.Process.ExitCode -ne 0) { throw 'Portable warm-cache preparation failed.' }
    } finally {
        if (-not $probe.Process.HasExited) { $probe.Process.Kill(); $probe.Process.WaitForExit() }
        $probe.Process.Dispose()
    }
}
$rows = [Collections.Generic.List[object]]::new()
$workloads = @('1920 1080 300 1', '1920 1080 180 4', '3840 2160 120 1', '3840 2160 80 4')
for ($round = 0; $round -lt $Runs; $round++) {
    $order = if ($round % 2 -eq 0) { @('before', 'after') } else { @('after', 'before') }
    foreach ($workload in $workloads) {
        foreach ($name in $order) {
            $result = Invoke-Conversion (Join-Path $variants[$name] 'frame-conversion.exe') $workload
            $rows.Add(@{ kind = 'conversion'; variant = $name; round = $round + 1; result = $result })
        }
    }
    foreach ($name in $order) {
        $result = Invoke-Wrapper (Join-Path $fixtureRoot "$name.exe")
        $rows.Add(@{ kind = 'wrapper'; variant = $name; round = $round + 1; result = $result })
    }
    Write-Host "Completed matched footprint round $($round + 1)/$Runs."
}
$report = @{ schema = 1; measuredAtUtc = [DateTime]::UtcNow.ToString('o'); runs = $Runs; hashes = $hashes; payloadSha256 = (Get-FileHash -LiteralPath $payloadPath).Hash.ToLowerInvariant(); payloadBytes = (Get-Item -LiteralPath $payloadPath).Length; rows = $rows.ToArray(); limits = 'Conversion-stage synthetic CPU buffers, no rendering/network/decoder throughput. Wrapper samples use identical real payload and a child blocked on stdin, no capture or room. Process CPU includes startup/warm-up; Rust allocation counters exclude C++/GPU allocations. Private bytes and working set are separate. No game, AMD or A/V qualification.' }
$report | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $outputPath -Encoding utf8
Write-Host "Saved matched measurements: $outputPath"
