param(
    [ValidateSet('check', 'test', 'build', 'run', 'probe', 'qualify')]
    [string]$Action = 'check',
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$NativeArguments
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$gstRoot = if ($env:FREEDOMSTREAM_GSTREAMER) { $env:FREEDOMSTREAM_GSTREAMER } else { Join-Path $repository '.local/gstreamer' }
if (-not (Test-Path -LiteralPath (Join-Path $gstRoot 'bin/gst-inspect-1.0.exe'))) {
    throw 'Native runtime is missing. Run npm run native:setup first.'
}
$env:PATH = "$(Join-Path $gstRoot 'bin');$env:PATH"
$env:PKG_CONFIG_PATH = Join-Path $gstRoot 'lib/pkgconfig'
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $gstRoot
Push-Location $repository
try {
    switch ($Action) {
        'check' {
            cargo fmt --all -- --check
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
            cargo clippy --locked --workspace --all-targets -- -D warnings
        }
        'test' { cargo test --locked --workspace }
        'build' { cargo build --locked --release }
        'run' { cargo run --locked --bin freedomstream-publisher -- @NativeArguments }
        'probe' { cargo run --locked --bin freedomstream-publisher -- --probe }
        'qualify' { cargo run --locked --bin freedomstream-publisher -- --qualify @NativeArguments }
    }
    exit $LASTEXITCODE
} finally { Pop-Location }
