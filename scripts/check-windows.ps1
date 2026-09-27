param([switch]$VerifySoftwareProfilePolling, [switch]$SkipDeviceDiscovery, [switch]$Gui)

$ErrorActionPreference = "Stop"
if ($VerifySoftwareProfilePolling) {
    throw 'Runtime hardware verification is suspended for device safety (2026-09-27). Use normal offline tests/build; see docs/research.md.'
}
$env:CARGO_INCREMENTAL = "0"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$target = "x86_64-pc-windows-msvc"

if (-not (Test-Path -LiteralPath $cargo)) {
    throw "Windows cargo.exe was not found at $cargo. Install Rust with rustup first."
}

Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)

$desktopFeatures = @()
if ($Gui) {
    if (-not (Test-Path -LiteralPath 'apps/hyperx-gui/dist/index.html')) {
        throw 'Build the frontend first: cd apps/hyperx-gui; npm ci; npm run build'
    }
    # Embed built assets: a native binary must not depend on a Vite dev server.
    $desktopFeatures = @('--features', 'hyperx-gui/desktop,hyperx-gui/custom-protocol')
}
& $cargo test --workspace --all-targets --locked --target $target @desktopFeatures
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $cargo build --workspace --locked --target $target @desktopFeatures
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

& $binary profile inspect-capture crates/hyperx-protocol/tests/fixtures/cold-legacy-startup-images.hex
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $binary profile diff-capture-images `
    crates/hyperx-protocol/tests/fixtures/read-request-get-onboard.hex `
    crates/hyperx-protocol/tests/fixtures/cold-legacy-startup-images.hex `
    --before-report 2 --after-report 2
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not $SkipDeviceDiscovery) {
    & $binary devices
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$tui = Join-Path (Get-Location) "target\$target\debug\hyperx-tui.exe"
& $tui --version
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $tui --demo --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $tui --demo --render
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $tui --demo --render --view macros
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $tui --demo --render --view profiles
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Gui) {
    $guiBinary = Join-Path (Get-Location) "target\$target\debug\hyperx-gui.exe"
    # Exercise the actual executable without a webview, USB or native dialogs.
    & $guiBinary --smoke-test
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# Explicitly opt in: normal CI/build checks never mutate a physical device.
if ($VerifySoftwareProfilePolling) {
    & (Join-Path $PSScriptRoot 'verify-software-profile-windows.ps1') -ConfirmRuntimeWrites
}
exit 0
