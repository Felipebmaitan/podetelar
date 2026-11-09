param([switch]$KeepDownloads)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Ownership.ps1')
Assert-FreedomStreamRoot $PSScriptRoot
$root = (Get-Item -LiteralPath $PSScriptRoot).FullName
$running = Get-CimInstance Win32_Process -Filter "Name = 'freedomstream-publisher.exe'" | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) }
if ($running) { throw 'Close the installed FreedomStream publisher before uninstalling.' }
$shortcutPath = Join-Path ([Environment]::GetFolderPath('Programs')) 'FreedomStream.lnk'
if ((Test-Path -LiteralPath (Join-Path $root '.start-menu-shortcut')) -and (Test-Path -LiteralPath $shortcutPath)) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    if ($shortcut.Arguments.Contains((Join-Path $root 'Start-FreedomStream.ps1'))) { Remove-Item -LiteralPath $shortcutPath }
}
if ($KeepDownloads) {
    foreach ($entry in Get-ChildItem -LiteralPath $root -Force) {
        if ($entry.Name -ne 'downloads') { Remove-Item -LiteralPath $entry.FullName -Recurse -Force }
    }
} else {
    # Assert-FreedomStreamRoot resolved/checked this exact absolute target above.
    Remove-Item -LiteralPath $root -Recurse -Force
}
Write-Output 'Removed FreedomStream. Shared Microsoft/GPU/Chrome installations were kept.'
