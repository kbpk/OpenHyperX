param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath,
    [switch]$ConfirmLegacyLaunch
)

# No OpenHyperX executable or HID handle. Legacy itself may write settings on
# launch. Do not close it automatically: closing can send a profile selector.
$ErrorActionPreference = 'Stop'
if (-not $ConfirmLegacyLaunch) {
    throw 'Explicit -ConfirmLegacyLaunch required: Legacy may apply its software state. Confirm a healthy cold reconnect and closed writers first.'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this wrapper unelevated; only its capture helper may request UAC. Legacy must not be launched as administrator.'
}
if (@(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' }).Count) {
    throw 'Close NGENUITY/OpenRGB including tray processes; no application was launched.'
}
# Exact Store Legacy identity observed locally, not a name search that might
# activate current NGENUITY or another package. Unknown versions need review.
$family = '33C30B79.HyperXNGenuity_0a78dr3hq0pvt'
$appId = "$family!App"
$packages = @(Get-AppxPackage -Name '33C30B79.HyperXNGenuity')
if ($packages.Count -ne 1 -or $packages[0].PackageFamilyName -ne $family -or
    "$($packages[0].Version)" -ne '5.38.0.0' -or
    @((Get-StartApps) | Where-Object { $_.AppID -eq $appId }).Count -ne 1) {
    throw 'Expected installed Microsoft Store NGENUITY Legacy 5.38.0.0 was not uniquely identified. No launch.'
}
$output = [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($OutputPath))
if ((Split-Path -Parent $output) -ne [IO.Path]::GetFullPath($env:TEMP).TrimEnd('\')) {
    throw 'Use a new capture filename directly under %TEMP%.'
}
$readyPath = "$output.ready.json"
$manifestPath = "$output.launch.json"
foreach ($path in @($output, $readyPath, $manifestPath)) {
    if (Test-Path -LiteralPath $path) { throw "Refusing to overwrite: $path" }
}
$captureScript = Join-Path $PSScriptRoot 'capture-windows.ps1'
if (-not (Test-Path -LiteralPath $captureScript)) { throw "Missing capture helper: $captureScript" }
New-Item -ItemType File -Path $readyPath | Out-Null
$manifest = [ordered]@{
    PackageFullName = $packages[0].PackageFullName
    AppID = $appId
    OperatorConfirmedHealthyColdState = $true
    LaunchUtc = $null
    CaptureReady = $null
    Outcome = 'Not launched'
}
$captureJob = Start-Job -ScriptBlock {
    param($script, $controller, $output, $ready)
    & $script -Interface $controller -VendorId 2385 -ProductId 5860 `
        -DurationSeconds 12 -OutputPath $output -ReadyPath $ready
    if ($LASTEXITCODE -ne 0) { throw "Capture helper exited with code $LASTEXITCODE." }
} -ArgumentList $captureScript, $Interface, $output, $readyPath
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    $ready = $null
    while ($captureJob.State -eq 'Running' -and [DateTime]::UtcNow -lt $deadline) {
        # A partial JSON write is merely an incomplete host handshake. This
        # loop neither retries capture nor communicates with USB.
        try { $ready = Get-Content -LiteralPath $readyPath -Raw | ConvertFrom-Json }
        catch { $ready = $null }
        if ($null -ne $ready) { break }
        Start-Sleep -Milliseconds 100
    }
    if ($null -eq $ready -or $captureJob.State -ne 'Running' -or
        $ready.Interface -ne $Interface -or $ready.DurationSeconds -ne 12 -or
        $ready.DeviceAddress -lt 1 -or $ready.DeviceAddress -gt 127) {
        throw 'Capture never reached a valid ready handshake. Legacy was not launched.'
    }
    $started = [DateTimeOffset]::Parse($ready.StartedUtc).UtcDateTime
    $age = ([DateTime]::UtcNow - $started).TotalSeconds
    if ($age -lt 0 -or $age -gt 2) { throw 'Capture readiness is stale. Legacy was not launched.' }
    Start-Sleep -Milliseconds 300
    # Recheck the only competing writers immediately before the one activation.
    if (@(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' }).Count) {
        throw 'A competing writer started during capture setup. Legacy was not launched by this script.'
    }
    $manifest.CaptureReady = $ready
    $manifest.LaunchUtc = [DateTime]::UtcNow.ToString('o')
    Write-Host 'Capturing: launching Store NGENUITY Legacy once. Do not click settings, updates or Save to mouse.' -ForegroundColor Cyan
    Start-Process -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList "shell:AppsFolder\$appId"
    $manifest.Outcome = 'Activation requested once; review capture and physical behavior'
}
catch {
    $manifest.Outcome = "Failed: $($_.Exception.Message)"
    throw
}
finally {
    $manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    Wait-Job -Job $captureJob -Timeout 20 | Out-Null
    Receive-Job -Job $captureJob
}
if ($captureJob.State -ne 'Completed') { throw "Capture helper failed: $($captureJob.State). Do not relaunch." }
Remove-Job -Job $captureJob
if (-not (Test-Path -LiteralPath $output) -or (Get-Item -LiteralPath $output).Length -le 24) {
    throw 'Capture missing/header-only. Inspect artifacts without repeating the launch.'
}
Write-Host 'Leave Legacy open and settings untouched until all captured traffic is reviewed.' -ForegroundColor Cyan
