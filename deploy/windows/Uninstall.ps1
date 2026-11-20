$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Ownership.ps1')
Assert-PodeTelarRoot $PSScriptRoot
$root = (Get-Item -LiteralPath $PSScriptRoot).FullName
$running = Get-CimInstance Win32_Process -Filter "Name = 'podetelar-publisher.exe'" | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase) }
if ($running) { throw 'Close the installed PodeTelar publisher before uninstalling.' }
$shortcutPath = Join-Path ([Environment]::GetFolderPath('Programs')) 'PodeTelar.lnk'
if ((Test-Path -LiteralPath (Join-Path $root '.start-menu-shortcut')) -and (Test-Path -LiteralPath $shortcutPath)) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    if ($shortcut.Arguments.Contains((Join-Path $root 'Start-PodeTelar.ps1'))) { Remove-Item -LiteralPath $shortcutPath }
}
# Assert-PodeTelarRoot resolved/checked this exact absolute target above.
Remove-Item -LiteralPath $root -Recurse -Force
Write-Output 'Removed PodeTelar. Shared Microsoft/GPU/Chrome installations were kept.'
