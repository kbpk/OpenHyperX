param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $arguments = @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass',
        '-File', "`"$PSCommandPath`"",
        '-Interface', "`"$Interface`"",
        '-OutputPath', "`"$OutputPath`""
    )
    $elevated = Start-Process powershell.exe -Verb RunAs -ArgumentList $arguments `
        -Wait -PassThru
    exit $elevated.ExitCode
}

$writers = @(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' })
if ($writers.Count -ne 0) {
    throw "Close NGENUITY and OpenRGB first: $($writers.ProcessName -join ', ')"
}
$repo = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $repo 'target\x86_64-pc-windows-msvc\debug\hyperx-cli.exe'
if (-not (Test-Path -LiteralPath $binary)) {
    throw 'Build the Windows CLI before running this diagnostic.'
}
$captureScript = Join-Path $PSScriptRoot 'capture-windows.ps1'
$captureJob = Start-Job -ScriptBlock {
    param($scriptPath, $captureInterface, $captureOutput)
    & $scriptPath -Interface $captureInterface -VendorId 2385 -ProductId 5860 `
        -DurationSeconds 8 -OutputPath $captureOutput
} -ArgumentList $captureScript, $Interface, $OutputPath

# Wait for the identity-resolving helper to start USBPcap; no vendor report is
# sent before recording is active. The only CLI command here is a known,
# non-persistent runtime selector, never an onboard save or raw replay.
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
if (-not $ready) {
    Wait-Job -Job $captureJob -Timeout 15 | Out-Null
    Receive-Job -Job $captureJob
    throw 'The capture did not reach the recording stage; no probe was sent.'
}
Start-Sleep -Milliseconds 300
Write-Host 'Recording one non-persistent save ACK probe.' -ForegroundColor Cyan
& $binary --trace profile check-save-ack
$probeExitCode = $LASTEXITCODE
Wait-Job -Job $captureJob -Timeout 15 | Out-Null
Receive-Job -Job $captureJob
if ($captureJob.State -ne 'Completed') {
    throw "The capture job did not finish successfully: $($captureJob.State)"
}
Remove-Job -Job $captureJob
Write-Host "Probe exit code: $probeExitCode. Inspect the capture before any further probe." `
    -ForegroundColor Cyan
Read-Host 'Press ENTER to close this window' | Out-Null
exit $probeExitCode
