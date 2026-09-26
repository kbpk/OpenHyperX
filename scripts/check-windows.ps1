param([switch]$VerifySoftwareProfilePolling, [switch]$SkipDeviceDiscovery)

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

& $binary profile validate examples/profiles/pulsefire-raid.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary profile inspect examples/profiles/pulsefire-raid.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary profile diff examples/profiles/pulsefire-raid.toml examples/profiles/pulsefire-raid.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary profile inspect-capture crates/hyperx-protocol/tests/fixtures/button4-ab-toggle.hex
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not $SkipDeviceDiscovery) {
    & $binary devices
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# Explicitly opt in: normal CI/build checks never mutate a physical device.
if ($VerifySoftwareProfilePolling) {
    & (Join-Path $PSScriptRoot 'verify-software-profile-windows.ps1') -ConfirmRuntimeWrites
}
exit 0
