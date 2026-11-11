# Pauses only a harness-owned process whose executable identity was supplied by
# that harness. No firewall, network configuration or global CPU load changes.
param(
    [Parameter(Mandatory=$true)][int]$TestProcessId,
    [Parameter(Mandatory=$true)][string]$ExpectedExecutable,
    [ValidateRange(100,5000)][int]$PauseMilliseconds = 3000
)
$ErrorActionPreference = 'Stop'
$testProcess = [Diagnostics.Process]::GetProcessById($TestProcessId)
$expectedPath = [IO.Path]::GetFullPath($ExpectedExecutable)
if (-not [string]::Equals($testProcess.MainModule.FileName, $expectedPath, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Refusing to pause an unrelated process.'
}
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class DiagnosticFixturePause {
    [DllImport("ntdll.dll")] public static extern int NtSuspendProcess(IntPtr process);
    [DllImport("ntdll.dll")] public static extern int NtResumeProcess(IntPtr process);
}
"@
$testHandle = $testProcess.Handle
if ([DiagnosticFixturePause]::NtSuspendProcess($testHandle) -ne 0) { throw 'Could not pause test process.' }
try { Start-Sleep -Milliseconds $PauseMilliseconds }
finally {
    if ([DiagnosticFixturePause]::NtResumeProcess($testHandle) -ne 0) { throw 'Could not resume test process.' }
    $testProcess.Dispose()
}
