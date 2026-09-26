$ErrorActionPreference = "Stop"
$env:CARGO_INCREMENTAL = "0"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$target = "x86_64-pc-windows-msvc"

if (-not (Test-Path -LiteralPath $cargo)) {
    throw "Windows cargo.exe was not found at $cargo. Install Rust with rustup first."
}

Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)

& $cargo test --workspace --all-targets --locked --target $target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $cargo build --workspace --locked --target $target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$binary = Join-Path (Get-Location) "target\$target\debug\hyperx-cli.exe"
& $binary --version
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary buttons capabilities
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary buttons validate-macro button4 examples/macros/ab-toggle-20ms.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary devices
exit $LASTEXITCODE
