$ErrorActionPreference = 'Stop'
# npm can inherit PowerShell 7's module paths before launching Windows
# PowerShell 5.1. Resolve built-in modules from the executing host first.
$env:PSModulePath = "$PSHOME\Modules;$env:PSModulePath"
$ProgressPreference = 'SilentlyContinue'
$repository = Split-Path -Parent $PSScriptRoot
$runtime = Get-Content -LiteralPath (Join-Path $repository 'deploy/native-runtime.json') -Raw | ConvertFrom-Json
$gstRoot = Join-Path $repository '.local/gstreamer'
$inspect = Join-Path $gstRoot 'bin/gst-inspect-1.0.exe'
if (Test-Path -LiteralPath $inspect) {
    $version = & $inspect --version
    $headers = Join-Path $gstRoot 'include/gstreamer-1.0/gst/gst.h'
    if ($LASTEXITCODE -ne 0 -or -not ($version -match [regex]::Escape($runtime.version)) -or -not (Test-Path -LiteralPath $headers)) { throw 'An incompatible or incomplete development runtime exists in .local/gstreamer. Inspect it before replacing it.' }
    Write-Output 'Kept the existing pinned GStreamer development runtime.'
    exit 0
}
$downloads = Join-Path $repository '.local/downloads'
New-Item -ItemType Directory -Force -Path $downloads | Out-Null
$installer = Join-Path $downloads "gstreamer-$($runtime.version).exe"
if (-not (Test-Path -LiteralPath $installer)) { Invoke-WebRequest -UseBasicParsing -TimeoutSec 600 -Uri $runtime.url -OutFile $installer }
$checksum = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($checksum -ne $runtime.sha256) { throw 'Native runtime checksum did not match the pinned upstream checksum.' }
$installation = Start-Process -FilePath $installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/CURRENTUSER', '/TYPE=devel', "/DIR=`"$gstRoot`"") -WindowStyle Hidden -PassThru
$installation.WaitForExit()
if ($installation.ExitCode -ne 0) { throw "Native runtime installer exited with code $($installation.ExitCode)." }
Write-Output 'Installed the pinned development runtime inside .local/gstreamer.'
