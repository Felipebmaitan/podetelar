param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
$repository = Split-Path -Parent $PSScriptRoot
$qualificationRoot = Join-Path $repository ('.local/portable-qualification-' + [guid]::NewGuid().ToString('N'))
$working = Join-Path $qualificationRoot 'A fresh directory with spaces'
$localData = Join-Path $qualificationRoot 'LocalAppData'
New-Item -ItemType Directory -Path $working, $localData | Out-Null
$copy = Join-Path $working 'FreedomStream.exe'
Copy-Item -LiteralPath $executablePath -Destination $copy
# Even a conflicting sibling Windows DLL must not be considered at startup.
[IO.File]::WriteAllText((Join-Path $working 'version.dll'), 'Not a DLL: launcher imports must use System32.')
$originalData = $env:LOCALAPPDATA
$originalPath = $env:PATH
$gstEnvironment = @{}
Get-ChildItem Env: | Where-Object { $_.Name -match '^GST_|^GSTREAMER_' } | ForEach-Object { $gstEnvironment[$_.Name] = $_.Value }

function Assert-GuiSubsystem([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($pe -lt 0 -or $pe + 94 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550 -or [BitConverter]::ToUInt16($bytes, $pe + 24 + 68) -ne 2) {
        throw 'A shipped executable does not use the Windows GUI subsystem.'
    }
}
Assert-GuiSubsystem $copy

function Start-Portable([string]$Arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $copy
    $info.Arguments = $Arguments
    $info.WorkingDirectory = $working
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    [void]$process.Start()
    return @{ Process = $process; Output = $process.StandardOutput.ReadToEndAsync(); Error = $process.StandardError.ReadToEndAsync() }
}
function Finish-Portable($Run, [bool]$Success = $true) {
    if (-not $Run.Process.WaitForExit(90000)) { throw 'Portable qualification timed out; retained for inspection.' }
    $output = $Run.Output.GetAwaiter().GetResult()
    $errorText = $Run.Error.GetAwaiter().GetResult()
    if ($Success -ne ($Run.Process.ExitCode -eq 0)) { throw "Unexpected portable exit $($Run.Process.ExitCode): $errorText" }
    $Run.Process.Dispose()
    return $output
}
try {
    $env:LOCALAPPDATA = $localData
    $env:PATH = "$env:WINDIR\System32;$env:WINDIR"
    $env:GST_PLUGIN_PATH_1_0 = 'C:\nonexistent-foreign-gstreamer'
    $env:GST_PLUGIN_SYSTEM_PATH_1_0 = 'C:\nonexistent-foreign-gstreamer'
    $env:GST_PLUGIN_SCANNER_1_0 = 'C:\nonexistent-foreign-gstreamer\scanner.exe'
    # Simultaneous first launches must share one complete cache.
    $first = Start-Portable '--probe'
    $second = Start-Portable '--probe'
    foreach ($run in @($first, $second)) {
        $probe = (Finish-Portable $run) | ConvertFrom-Json
        if ($probe.runtime -notmatch '^GStreamer 1\.28\.8') { throw 'Unexpected bundled runtime.' }
    }
    $fallback = (Finish-Portable (Start-Portable '--bundled-probe')) | ConvertFrom-Json
    if ($fallback.runtime -notmatch '^GStreamer 1\.28\.8') { throw 'Bundled CRT fallback did not start.' }
    $cache = @(Get-ChildItem -LiteralPath (Join-Path $localData 'FreedomStreamPortable') -Directory | Where-Object { $_.Name -match '^[a-f0-9]{64}$' })
    if ($cache.Count -ne 1) { throw 'First launches did not converge on one cache.' }
    $runtime = $cache[0].FullName
    Assert-GuiSubsystem (Join-Path $runtime 'bin/freedomstream-publisher.exe')
    $manifest = Get-Content -LiteralPath (Join-Path $runtime 'release.json') -Raw | ConvertFrom-Json
    foreach ($file in $manifest.files.PSObject.Properties) {
        $path = Join-Path $runtime $file.Name
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.Value) { throw "Extracted file mismatch: $($file.Name)" }
    }
    $inputs = Get-Content -LiteralPath (Join-Path $runtime 'notices/runtime-inputs.json') -Raw | ConvertFrom-Json
    foreach ($source in $inputs.sources) {
        $path = Join-Path $runtime ('notices/sources/' + $source.filename)
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $source.sha256) { throw 'Corresponding-source archive mismatch.' }
    }
    $marker = Join-Path $runtime 'user-modification.txt'
    [IO.File]::WriteAllText($marker, 'retain local changes')
    [void](Finish-Portable (Start-Portable '--probe'))
    if ((Get-Content -LiteralPath $marker -Raw) -ne 'retain local changes') { throw 'A completed cache was overwritten.' }
    $extracted = Join-Path $qualificationRoot 'Explicit extraction'
    [void](Finish-Portable (Start-Portable "--extract `"$extracted`""))
    if (-not (Test-Path -LiteralPath (Join-Path $extracted 'notices/sources/README.md'))) { throw 'Explicit extraction omitted sources.' }
    [void](Finish-Portable (Start-Portable "--extract `"$extracted`"") $false)
    $junction = Join-Path $qualificationRoot 'Redirected AppData'
    New-Item -ItemType Junction -Path $junction -Target $localData | Out-Null
    $env:LOCALAPPDATA = $junction
    [void](Finish-Portable (Start-Portable '--probe') $false)
    $env:LOCALAPPDATA = $localData
    # Reject damage before attempting extraction; no UI dialog for CLI failures.
    $stream = [IO.File]::Open($copy, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite)
    try {
        [void]$stream.Seek(-57, [IO.SeekOrigin]::End)
        $value = $stream.ReadByte()
        [void]$stream.Seek(-1, [IO.SeekOrigin]::Current)
        $stream.WriteByte($value -bxor 1)
    } finally { $stream.Dispose() }
    [void](Finish-Portable (Start-Portable '--probe') $false)
    Write-Host 'Portable qualification passed: isolated-path first launch, concurrent extraction, runtime probe, payload/source integrity, cache preservation, explicit extraction and damage rejection. This is not a fresh-PC or disconnected-network test.'
    Write-Host "Retained qualification evidence: $qualificationRoot"
} finally {
    $env:LOCALAPPDATA = $originalData
    $env:PATH = $originalPath
    Get-ChildItem Env: | Where-Object { $_.Name -match '^GST_|^GSTREAMER_' } | ForEach-Object { Remove-Item -LiteralPath ('Env:' + $_.Name) }
    foreach ($name in $gstEnvironment.Keys) { Set-Item -LiteralPath ('Env:' + $name) -Value $gstEnvironment[$name] }
}
