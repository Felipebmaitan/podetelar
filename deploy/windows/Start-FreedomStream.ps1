$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Ownership.ps1')
Assert-FreedomStreamRoot $PSScriptRoot
$version = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'current.json') -Raw | ConvertFrom-Json
if ($version -notmatch '^\d+\.\d+\.\d+-[a-f0-9]{12}$') { throw 'Invalid installed version.' }
$root = Join-Path $PSScriptRoot "versions/$version"
$env:PATH = "$(Join-Path $root 'runtime/bin');$env:WINDIR\System32;$env:WINDIR"
$env:GST_PLUGIN_PATH_1_0 = ''
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = Join-Path $root 'runtime/lib/gstreamer-1.0'
$env:GST_PLUGIN_SCANNER_1_0 = Join-Path $root 'runtime/libexec/gstreamer-1.0/gst-plugin-scanner.exe'
$env:GST_REGISTRY_1_0 = Join-Path $root 'registry.bin'
Start-Process -FilePath (Join-Path $root 'runtime/bin/freedomstream-publisher.exe') -WorkingDirectory $root -WindowStyle Hidden
