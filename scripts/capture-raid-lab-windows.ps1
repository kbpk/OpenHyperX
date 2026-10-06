param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath,
    [switch]$ConfirmPassiveGet,
    [switch]$ConfirmReadRequestGet,
    [switch]$ConfirmRuntimeSelector
)

# Three fixed experiments, each with separate consent. The selector-only mode
# has disabled input/lighting in a real test. The standalone 81 request also
# sends a SET_REPORT, and its physical effects are not fully established; none
# of these probes is guaranteed harmless. No settings/firmware writes,
# initializer, persistent save, retry or software recovery.
$ErrorActionPreference = 'Stop'
$choices = @(
    if ($ConfirmPassiveGet) { 'raid-feature-get' }
    if ($ConfirmReadRequestGet) { 'raid-read-request-get' }
    if ($ConfirmRuntimeSelector) { 'raid-runtime-select-only' }
)
if ($choices.Count -ne 1) {
    throw 'Choose exactly one consent: -ConfirmPassiveGet (GET only), -ConfirmReadRequestGet (81 then GET), or -ConfirmRuntimeSelector (potentially disruptive 03 only).'
}
$labCommand = $choices[0]
$consentFlag = switch ($labCommand) {
    'raid-feature-get' { '-ConfirmPassiveGet' }
    'raid-read-request-get' { '-ConfirmReadRequestGet' }
    'raid-runtime-select-only' { '-ConfirmRuntimeSelector' }
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"",
        '-Interface', "`"$Interface`"", '-OutputPath', "`"$OutputPath`"", $consentFlag)
    $elevated = Start-Process powershell.exe -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    exit $elevated.ExitCode
}
if (@(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' }).Count) {
    throw 'Close NGENUITY/OpenRGB including tray processes; no lab probe was started.'
}
$repo = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $repo 'target\x86_64-pc-windows-msvc\debug\hyperx-cli.exe'
$captureScript = Join-Path $PSScriptRoot 'capture-windows.ps1'
foreach ($file in @($binary, $captureScript)) {
    if (-not (Test-Path -LiteralPath $file)) { throw "Missing dependency: $file" }
}
$output = [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($OutputPath))
if (-not (Test-Path -LiteralPath (Split-Path -Parent $output) -PathType Container)) {
    throw 'Use an existing capture directory, preferably %TEMP%.'
}
$stdout = "$output.stdout.log"
$stderr = "$output.stderr.log"
$failureLog = "$output.failure.log"
foreach ($path in @($output, $stdout, $stderr, $failureLog)) {
    if (Test-Path -LiteralPath $path) { throw "Refusing to overwrite: $path" }
}

# Verify the NEW fixed command exists before starting a capture; this is help
# only, so even an old executable cannot issue an accidental runtime query.
& $binary lab $labCommand --help | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Build the CLI with the selected isolated lab command first.' }

$captureJob = Start-Job -ScriptBlock {
    param($script, $captureInterface, $captureOutput)
    & $script -Interface $captureInterface -VendorId 2385 -ProductId 5860 `
        -DurationSeconds 5 -OutputPath $captureOutput
} -ArgumentList $captureScript, $Interface, $output
$probe = $null
try {
    $ready = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ($captureJob.State -eq 'Running' -and [DateTime]::UtcNow -lt $deadline) {
        $messages = @(Receive-Job -Job $captureJob -Keep)
        if (@($messages | Where-Object { "$_" -match '^Capturing ' }).Count) { $ready = $true; break }
        Start-Sleep -Milliseconds 100
    }
    if (-not $ready -or $captureJob.State -ne 'Running') { throw 'Capture did not reach recording; no lab probe was started.' }
    Start-Sleep -Milliseconds 300
    $description = switch ($labCommand) {
        'raid-feature-get' { 'One GET_REPORT only: no SET_REPORT, selector, initializer, RGB or onboard writes.' }
        'raid-read-request-get' { 'One fixed SET_REPORT 07 81, wait 110 ms, then one GET_REPORT. Physical effects are not fully established: have USB reconnect available. No selector, initializer, RGB or onboard writes.' }
        'raid-runtime-select-only' { 'WARNING: one SET_REPORT 07 03 04 64 only; MAY disable cursor/clicks/lighting. NO request 81, GET, initializer, recovery or onboard write.' }
    }
    Write-Host $description -ForegroundColor Cyan
    $probe = Start-Process -FilePath $binary -ArgumentList @('--trace', 'lab', $labCommand, '--unsafe') `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru -NoNewWindow
    # Windows PowerShell 5.1 can otherwise leave ExitCode null after the
    # redirected child exits (reproduced offline using only --help). Retain
    # its process handle before waiting; null is a host error, never success.
    $null = $probe.Handle
    if (-not $probe.WaitForExit(3000)) {
        Stop-Process -Id $probe.Id
        $probe.WaitForExit()
        throw 'Lab probe exceeded the host time bound. STOP without retry, recovery or another report.'
    }
    $probe.WaitForExit()
    if ($null -eq $probe.ExitCode) { throw 'CLI exit code unavailable. STOP; inspect capture, never retry the probe.' }
    if ($probe.ExitCode -ne 0) { throw "Lab probe failed ($($probe.ExitCode)). STOP; inspect logs and capture." }
}
catch {
    # Elevated console output can disappear on exit. Preserve the original
    # failure outside Git so investigating it never requires another USB probe.
    $_ | Out-String | Set-Content -LiteralPath $failureLog -Encoding UTF8
    throw
}
finally {
    # Only the exact child process may be stopped; never kill a broad peripheral
    # process set. The identity-resolving capture helper ends itself after 5 s.
    if ($null -ne $probe -and -not $probe.HasExited) {
        Stop-Process -Id $probe.Id
        $probe.WaitForExit()
    }
    Wait-Job -Job $captureJob -Timeout 15 | Out-Null
    Receive-Job -Job $captureJob
    foreach ($log in @($stdout, $stderr, $failureLog)) {
        if (Test-Path -LiteralPath $log) {
            & icacls.exe $log /grant "$($identity.Name):(F)" | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Failed to grant access to lab log: $log" }
            Get-Content -LiteralPath $log
        }
    }
}
if ($captureJob.State -ne 'Completed') { throw "Capture failed: $($captureJob.State). No further report will be sent." }
Remove-Job -Job $captureJob
if (-not (Test-Path -LiteralPath $output) -or (Get-Item -LiteralPath $output).Length -le 24) {
    throw 'Capture is missing/header-only. Inspect existing output; do not repeat automatically.'
}
Write-Host 'Lab probe finished. Review all target transfers and ask for physical behavior before any next experiment.' -ForegroundColor Cyan
exit 0
