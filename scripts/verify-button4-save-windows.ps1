param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [Parameter(Mandatory = $true)]
    [ValidateSet('save-ab', 'restore-back')]
    [string]$Action,

    [switch]$ConfirmSave
)

# Fixed-purpose lab check for the locally recorded Raid release-1124 state:
# Button 5 has coverage-recorded-timing, wheel is off, logo is blue. This is
# not a general profile importer and never exposes raw reports or firmware I/O.
$ErrorActionPreference = 'Stop'
if (-not $ConfirmSave) { throw 'This writes onboard memory. Pass -ConfirmSave explicitly.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $arguments = @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"",
        '-Interface', "`"$Interface`"", '-OutputPath', "`"$OutputPath`"",
        '-Action', $Action, '-ConfirmSave'
    )
    $elevated = Start-Process powershell.exe -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    exit $elevated.ExitCode
}
$writers = @(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' })
if ($writers.Count -ne 0) { throw "Close competing writers: $($writers.ProcessName -join ', ')" }
$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
$binary = Join-Path $repo 'target\x86_64-pc-windows-msvc\debug\hyperx-cli.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Build the native Windows CLI first.' }
$captureScript = Join-Path $PSScriptRoot 'capture-windows.ps1'
$captureJob = Start-Job -ScriptBlock {
    param($scriptPath, $captureInterface, $captureOutput)
    & $scriptPath -Interface $captureInterface -VendorId 2385 -ProductId 5860 `
        -DurationSeconds 12 -OutputPath $captureOutput
} -ArgumentList $captureScript, $Interface, $OutputPath

$failure = $null
try {
    $ready = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ($captureJob.State -eq 'Running' -and [DateTime]::UtcNow -lt $deadline) {
        $output = @(Receive-Job -Job $captureJob -Keep)
        if (@($output | Where-Object { "$_" -match '^Capturing ' }).Count -ne 0) {
            $ready = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $ready) { throw 'Capture did not start. No mouse operation was attempted.' }
    Start-Sleep -Milliseconds 300
    Write-Host "Recording one persistent $Action transaction. Do not click mouse buttons." -ForegroundColor Cyan
    & $binary --trace info
    if ($LASTEXITCODE -ne 0) { throw 'Baseline read failed; no write was attempted.' }
    if ($Action -eq 'save-ab') {
        & $binary --trace buttons set button4 macro examples/macros/ab-20ms.toml
    }
    else {
        & $binary --trace buttons set button4 mouse back
    }
    if ($LASTEXITCODE -ne 0) { throw 'Runtime assignment failed. No persistent save or retry was attempted.' }
    # Captured runtime profile ACKs arrive roughly 65 ms after TX. Let that
    # separate volatile command settle before opening the save ACK handle.
    Start-Sleep -Milliseconds 150

    $saveArguments = @(
        '--trace', 'profile', 'save-to-mouse', '--wheel', 'off', '--logo', '0000FF',
        '--macro-definition', 'button5=examples/macros/coverage-recorded-timing.toml', '--confirm'
    )
    if ($Action -eq 'save-ab') {
        $saveArguments += @('--macro-definition', 'button4=examples/macros/ab-20ms.toml')
    }
    & $binary @saveArguments
    if ($LASTEXITCODE -ne 0) { throw 'Persistent save failed. STOP: do not retry or automatically restore.' }
    & $binary --trace info
    if ($LASTEXITCODE -ne 0) { throw 'Save completed, but independent runtime read failed. STOP before any further write.' }
}
catch { $failure = $_ }
finally {
    Wait-Job -Job $captureJob -Timeout 15 | Out-Null
    try {
        Receive-Job -Job $captureJob
        if ($captureJob.State -ne 'Completed') { throw "Capture failed: $($captureJob.State)" }
    }
    catch { if ($null -eq $failure) { $failure = $_ } }
    if ($captureJob.State -ne 'Running') { Remove-Job -Job $captureJob }
}
if ($null -ne $failure) {
    Write-Host "STOP: $failure" -ForegroundColor Red
    Read-Host 'Press ENTER to close; inspect all captured traffic before any next operation' | Out-Null
    exit 1
}
Write-Host 'Save and runtime read completed. Wait for capture review before reconnecting USB.' -ForegroundColor Cyan
Read-Host 'Press ENTER to close this window' | Out-Null
exit 0
