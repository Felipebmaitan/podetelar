param([string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'FreedomStream'), [switch]$NoShortcut, [string]$RuntimeArchive)
$ErrorActionPreference = 'Stop'
$env:PSModulePath = "$PSHOME\Modules;$env:PSModulePath"
$ProgressPreference = 'SilentlyContinue'
. (Join-Path $PSScriptRoot 'Ownership.ps1')
if (-not [Environment]::Is64BitOperatingSystem -or [Environment]::OSVersion.Version.Build -lt 20348) { throw 'FreedomStream needs Windows x64 build 20348 or later (Windows 11 recommended).' }
$release = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'release.json') -Raw | ConvertFrom-Json
if ($release.id -notmatch '^\d+\.\d+\.\d+-[a-f0-9]{12}$') { throw 'Invalid release identifier.' }
foreach ($entry in $release.files.PSObject.Properties) {
    if ($entry.Name -match '(^|[\\/])\.\.([\\/]|$)|^[\\/]|:' -or $entry.Value -notmatch '^[a-f0-9]{64}$') { throw 'Invalid release file.' }
    if ((Get-FileHash -LiteralPath (Join-Path $PSScriptRoot $entry.Name) -Algorithm SHA256).Hash -ne $entry.Value) { throw "Damaged package file: $($entry.Name)" }
}
$InstallRoot = [IO.Path]::GetFullPath($InstallRoot)
if (Test-Path -LiteralPath $InstallRoot) { Assert-FreedomStreamRoot $InstallRoot }
else {
    New-Item -ItemType Directory -Path $InstallRoot | Out-Null
    Set-Content -LiteralPath (Join-Path $InstallRoot '.freedomstream-install') -Value 'FreedomStream per-user installation v1' -Encoding ASCII
    Assert-FreedomStreamRoot $InstallRoot
}
$versionRoot = Join-Path $InstallRoot "versions/$($release.id)"
if (Test-Path -LiteralPath $versionRoot) { throw 'This release is already installed or an interrupted installation exists. Keep the current version; inspect the version directory before retrying.' }
New-Item -ItemType Directory -Path $versionRoot -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Ownership.ps1') -Destination $InstallRoot -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Uninstall.ps1') -Destination $InstallRoot -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Start-FreedomStream.ps1') -Destination $InstallRoot -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'release.json') -Destination $versionRoot
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'notices') -Destination $versionRoot -Recurse
$runtime = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'native-runtime.json') -Raw | ConvertFrom-Json
$downloads = Join-Path $InstallRoot 'downloads'
New-Item -ItemType Directory -Force -Path $downloads | Out-Null
$installer = Join-Path $downloads "gstreamer-$($runtime.version).exe"
if ($RuntimeArchive) { Copy-Item -LiteralPath $RuntimeArchive -Destination $installer }
if (-not (Test-Path -LiteralPath $installer)) { Invoke-WebRequest -UseBasicParsing -TimeoutSec 600 -Uri $runtime.url -OutFile $installer }
if ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ne $runtime.sha256) { throw 'GStreamer download checksum mismatch; no installer was run.' }
# Upstream portable mode disables registry/environment/uninstall registration and
# VC redist tasks. Do not install over or alter another GStreamer installation.
$gstRoot = Join-Path $versionRoot 'runtime'
$process = Start-Process -FilePath $installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/CURRENTUSER', '/portable=1', '/TYPE=runtime', '/TASKS=""', "/DIR=`"$gstRoot`"") -WindowStyle Hidden -PassThru
$process.WaitForExit()
if ($process.ExitCode -ne 0) { throw "Runtime extraction failed ($($process.ExitCode)). The current installed release was kept." }
New-Item -ItemType Directory -Force -Path (Join-Path $gstRoot 'share') | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'notices/gstreamer') -Destination (Join-Path $gstRoot 'share/licenses') -Recurse
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'notices/gstreamer-versions.txt') -Destination (Join-Path $gstRoot 'share/versions.txt')
$missingCrt = @('vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll') | Where-Object { -not (Test-Path -LiteralPath (Join-Path $env:WINDIR "System32/$_")) }
if ($missingCrt.Count -gt 0) {
    $crt = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'visual-cpp.json') -Raw | ConvertFrom-Json
    $crtInstaller = Join-Path $downloads 'VC_redist.x64.exe'
    if (-not (Test-Path -LiteralPath $crtInstaller)) { Invoke-WebRequest -UseBasicParsing -TimeoutSec 600 -Uri $crt.url -OutFile $crtInstaller }
    if ((Get-FileHash -LiteralPath $crtInstaller -Algorithm SHA256).Hash -ne $crt.sha256) { throw 'Microsoft runtime checksum mismatch.' }
    $signature = Get-AuthenticodeSignature -LiteralPath $crtInstaller
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation,') { throw 'Microsoft runtime signature verification failed.' }
    # Only this shared Microsoft prerequisite needs the user's normal Windows UAC.
    $process = Start-Process -FilePath $crtInstaller -ArgumentList @('/install', '/passive', '/norestart') -Verb RunAs -PassThru
    $process.WaitForExit()
    if ($process.ExitCode -notin @(0, 1638, 3010)) { throw "Microsoft runtime installation failed ($($process.ExitCode))." }
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'freedomstream-publisher.exe') -Destination (Join-Path $gstRoot 'bin')
$env:PATH = "$(Join-Path $gstRoot 'bin');$env:WINDIR\System32;$env:WINDIR"
$env:GST_PLUGIN_PATH_1_0 = ''
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = Join-Path $gstRoot 'lib/gstreamer-1.0'
$env:GST_PLUGIN_SCANNER_1_0 = Join-Path $gstRoot 'libexec/gstreamer-1.0/gst-plugin-scanner.exe'
$env:GST_REGISTRY_1_0 = Join-Path $versionRoot 'registry.bin'
$probe = & (Join-Path $gstRoot 'bin/freedomstream-publisher.exe') --probe
if ($LASTEXITCODE -ne 0 -or -not ($probe -match 'GStreamer 1\.28\.8')) { throw 'Packaged runtime probe failed. The previous version remains current. Check the Visual C++ x64 prerequisite and GPU driver.' }
Set-Content -LiteralPath (Join-Path $InstallRoot 'current.json.new') -Value ($release.id | ConvertTo-Json) -Encoding ASCII
Move-Item -LiteralPath (Join-Path $InstallRoot 'current.json.new') -Destination (Join-Path $InstallRoot 'current.json') -Force
if (-not $NoShortcut) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Programs')) 'FreedomStream.lnk'))
    $shortcut.TargetPath = "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
    $shortcut.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$(Join-Path $InstallRoot 'Start-FreedomStream.ps1')`""
    $shortcut.WorkingDirectory = $InstallRoot
    $shortcut.IconLocation = Join-Path $gstRoot 'bin/freedomstream-publisher.exe'
    $shortcut.Save()
    Set-Content -LiteralPath (Join-Path $InstallRoot '.start-menu-shortcut') -Value 'FreedomStream.lnk' -Encoding ASCII
}
Write-Output "Installed FreedomStream $($release.id). Open it from the Start menu. To remove it, run $InstallRoot\Uninstall.ps1."
