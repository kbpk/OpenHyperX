$ErrorActionPreference = "Stop"
$env:CARGO_INCREMENTAL = "0"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$cargoArguments = $args

if (-not (Test-Path -LiteralPath $cargo)) {
    throw "Windows cargo.exe was not found at $cargo. Install Rust with rustup first."
}

Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
& $cargo @CargoArguments
exit $LASTEXITCODE
