$ErrorActionPreference = "Stop"
$env:CARGO_INCREMENTAL = "0"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$target = "x86_64-pc-windows-msvc"

if (-not (Test-Path -LiteralPath $cargo)) {
    throw "Windows cargo.exe was not found at $cargo. Install Rust with rustup first."
}

Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)

& $cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $cargo clippy --workspace --all-targets --target $target -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $cargo test --workspace --target $target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $cargo build --workspace --target $target
exit $LASTEXITCODE
