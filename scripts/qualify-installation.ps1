param([Parameter(Mandatory=$true)][string]$PackageDirectory)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$root = Join-Path $repository ('.local/install-qualification-' + [Guid]::NewGuid().ToString('N'))
$runtimeArchive = Join-Path $repository '.local/downloads/gstreamer-1.28.8.exe'
function GStreamerRegistration {
    $items = @(Get-ItemProperty 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'GStreamer' } | Select-Object PSChildName,DisplayVersion,InstallLocation,UninstallString)
    return ($items | ConvertTo-Json -Compress) + [Environment]::GetEnvironmentVariable('GSTREAMER_1_0_ROOT_MSVC_X86_64','User')
}
$before = GStreamerRegistration
& (Join-Path $PackageDirectory 'Install.ps1') -InstallRoot $root -NoShortcut -RuntimeArchive $runtimeArchive | Out-Null
if ((GStreamerRegistration) -ne $before) { throw 'Portable installation changed existing GStreamer registration.' }
$id = Get-Content -LiteralPath (Join-Path $root 'current.json') -Raw | ConvertFrom-Json
$runtime = Join-Path $root "versions/$id/runtime"
$env:PATH = "$(Join-Path $runtime 'bin');$env:WINDIR\System32;$env:WINDIR"
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = Join-Path $runtime 'lib/gstreamer-1.0'
$env:GST_PLUGIN_PATH_1_0 = ''
$probe = & (Join-Path $runtime 'bin/freedomstream-publisher.exe') --probe
if ($LASTEXITCODE -ne 0) { throw 'Installed publisher probe failed.' }
$result = $probe | ConvertFrom-Json
if ($result.runtime -notmatch 'GStreamer 1\.28\.8') { throw 'Wrong packaged runtime.' }
if (-not (Test-Path -LiteralPath (Join-Path $runtime 'share/licenses'))) { throw 'Missing runtime notices.' }
. (Join-Path $root 'Ownership.ps1')
try { Assert-FreedomStreamRoot $repository; throw 'An unrelated directory was accepted.' } catch { if ($_.Exception.Message -eq 'An unrelated directory was accepted.') { throw } }
& (Join-Path $root 'Uninstall.ps1') | Out-Null
if (Test-Path -LiteralPath $root) { throw 'Uninstall left its installation behind.' }
if ((GStreamerRegistration) -ne $before) { throw 'Uninstall changed existing GStreamer registration.' }
Write-Output 'Installation, isolated runtime probe, upstream registration preservation, ownership rejection and uninstallation passed. This is not a fresh-PC hardware qualification.'
