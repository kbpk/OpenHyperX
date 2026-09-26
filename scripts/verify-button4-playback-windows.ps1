param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [Parameter(Mandatory = $true)]
    [string]$CapturePrefix
)

# Fixed-purpose, volatile-only lab test. No physical macro execution, onboard
# save, raw-send facility, automatic retry or restoration after an ambiguous failure.
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $arguments = @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"",
        '-Interface', "`"$Interface`"", '-CapturePrefix', "`"$CapturePrefix`""
    )
    $elevated = Start-Process powershell.exe -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    exit $elevated.ExitCode
}
$prefix = [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($CapturePrefix))
if (-not (Test-Path -LiteralPath (Split-Path -Parent $prefix) -PathType Container)) {
    throw 'Use an existing capture directory, preferably %TEMP%.'
}
if (@(Get-ChildItem -LiteralPath (Split-Path -Parent $prefix) -Filter "$(Split-Path -Leaf $prefix)-*").Count) {
    throw 'Refusing to overwrite an existing test prefix.'
}
if (@(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' }).Count) {
    throw 'Close NGENUITY and OpenRGB before this test.'
}
$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
$binary = Join-Path $repo 'target\x86_64-pc-windows-msvc\debug\hyperx-cli.exe'
$tshark = Join-Path $env:ProgramFiles 'Wireshark\tshark.exe'
$captureScript = Join-Path $PSScriptRoot 'capture-windows.ps1'
foreach ($file in @($binary, $tshark, $captureScript)) {
    if (-not (Test-Path -LiteralPath $file)) { throw "Missing test dependency: $file" }
}
$results = [Collections.Generic.List[object]]::new()
$status = 'running'
$failure = $null
function Grant-TestOutputAccess([string]$Path) {
    & icacls.exe $Path /grant "$($identity.Name):(F)" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Failed to grant access to test output: $Path" }
}
function Write-TestResult {
    @{ status = $status; error = $failure; steps = @($results) } |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$prefix-result.json" -Encoding UTF8
    Grant-TestOutputAccess "$prefix-result.json"
}
function Invoke-TestCli([string[]]$Arguments) {
    # Native tracing uses stderr. Capture both streams without treating an
    # otherwise successful trace line as a PowerShell terminating error.
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = @(& $binary @Arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
    }
    finally { $ErrorActionPreference = $previousPreference }
    $clean = ($output -join "`n") -replace '\x1b\[[0-9;]*m', ''
    if ($code -ne 0) { throw "CLI failed ($code): $clean" }
    return $clean
}
function Get-ProfileHex([string]$Output) {
    $matches = [regex]::Matches($Output, 'direction="RX" interface=1 report_id=0x07 bytes=([0-9A-F ]+)')
    if ($matches.Count -ne 1) { throw 'Expected exactly one complete runtime profile read.' }
    $hex = $matches[0].Groups[1].Value.Trim().Replace(' ', '').ToLowerInvariant()
    if ($hex.Length -ne 528 -or -not $hex.StartsWith('078104')) { throw 'Unexpected runtime profile format.' }
    return $hex
}
try {
    $baselineOutput = Invoke-TestCli @('--trace', 'info')
    if ($baselineOutput -notmatch 'Release: 0x1124') { throw 'This lab test is limited to release 1124.' }
    $baseline = Get-ProfileHex $baselineOutput
    if ($baseline.Substring(0x88 * 2, 8) -ne '02f80003') { throw 'Baseline Button 4 must be Back. No assignment was attempted.' }
    $baselineOutput | Set-Content -LiteralPath "$prefix-baseline.log" -Encoding UTF8
    Grant-TestOutputAccess "$prefix-baseline.log"
    # Verified volatile startup restores ACKs after USB power loss. No profile,
    # lighting or macro write is performed by this diagnostic.
    Invoke-TestCli @('profile', 'check-save-ack', '--initialize-session') | Out-Null

    foreach ($phase in @('toggle', 'hold', 'once', 'back')) {
        $path = "$prefix-$phase.pcapng"
        $job = Start-Job -ScriptBlock {
            param($scriptPath, $captureInterface, $captureOutput)
            & $scriptPath -Interface $captureInterface -VendorId 2385 -ProductId 5860 `
                -DurationSeconds 8 -OutputPath $captureOutput
        } -ArgumentList $captureScript, $Interface, $path
        try {
            $ready = $false
            $deadline = [DateTime]::UtcNow.AddSeconds(20)
            while ($job.State -eq 'Running' -and [DateTime]::UtcNow -lt $deadline) {
                if (@(Receive-Job $job -Keep | Where-Object { "$_" -match '^Capturing ' }).Count) {
                    $ready = $true
                    break
                }
                Start-Sleep -Milliseconds 100
            }
            if (-not $ready) { throw 'Capture did not start; no assignment attempted.' }
            Start-Sleep -Milliseconds 300
            Write-Host "Testing runtime $phase; do not press mouse buttons." -ForegroundColor Cyan
            $arguments = @('--trace', 'buttons', 'set', 'button4')
            switch ($phase) {
                'toggle' { $arguments += @('macro', 'examples/macros/ab-toggle-20ms.toml') }
                'hold' { $arguments += @('macro', 'examples/macros/ab-hold-20ms.toml') }
                'once' { $arguments += @('macro', 'examples/macros/ab-20ms.toml') }
                'back' { $arguments += @('mouse', 'back') }
            }
            $txOutput = Invoke-TestCli $arguments
            $txOutput | Set-Content -LiteralPath "$prefix-$phase.log" -Encoding UTF8
            Grant-TestOutputAccess "$prefix-$phase.log"
            Start-Sleep -Milliseconds 150
            $readOutput = Invoke-TestCli @('--trace', 'info')
            $readOutput | Add-Content -LiteralPath "$prefix-$phase.log" -Encoding UTF8
            $readback = Get-ProfileHex $readOutput
            $record = if ($phase -eq 'back') { '02f80003' } else { '53000003' }
            $expected = $baseline.Substring(0, 0x88 * 2) + $record + $baseline.Substring(0x8C * 2)
            if ($readback -ne $expected) { throw "Unexpected profile changes after $phase. STOP without retry." }
        }
        finally {
            Wait-Job $job -Timeout 15 | Out-Null
            Receive-Job $job | Out-Host
            if ($job.State -ne 'Completed') { throw "Capture failed: $($job.State)" }
            Remove-Job $job
        }

        # Compare the complete captured feature writes against CLI trace and
        # require the exact ACK for every opcode before proceeding to another mode.
        $rows = @(& $tshark -r $path -Y 'usbhid.setup.bRequest == 9 || (usb.endpoint_address == 0x83 && usb.data_len > 0)' `
            -T fields -e usb.data_fragment -e usbhid.data)
        if ($LASTEXITCODE -ne 0) { throw 'Capture analysis failed. STOP.' }
        $capturedTx = @()
        $acks = @()
        foreach ($row in $rows) {
            $parts = $row -split "`t"
            if ($parts[0]) { $capturedTx += $parts[0].ToLowerInvariant() }
            if ($parts.Count -gt 1 -and $parts[1].StartsWith('000007')) { $acks += $parts[1].ToLowerInvariant() }
        }
        $tracedTx = @([regex]::Matches("$txOutput`n$readOutput", 'direction="TX" interface=1 report_id=0x07 bytes=([0-9A-F ]+)') |
            ForEach-Object { $_.Groups[1].Value.Trim().Replace(' ', '').ToLowerInvariant() })
        $expectedAcks = @($tracedTx | ForEach-Object {
            $opcode = $_.Substring(2, 2)
            $statusByte = if ($opcode -eq '81') { 'ff' } else { '00' }
            "000007${opcode}${statusByte}000000"
        })
        if (($capturedTx -join ',') -ne ($tracedTx -join ',') -or
            ($acks -join ',') -ne ($expectedAcks -join ',') -or $tracedTx.Count -lt 5) {
            throw "Unexpected TX/ACK sequence during $phase. STOP; no automatic restoration."
        }
        $results.Add(@{ phase = $phase; capture = $path; bytes = (Get-Item $path).Length; acknowledgements = $acks.Count; profile_preserved = $true })
        Write-TestResult
    }
    $status = 'passed'
}
catch { $failure = "$_"; $status = 'failed' }
Write-TestResult
if ($status -ne 'passed') {
    Write-Host "STOP: $failure" -ForegroundColor Red
    exit 1
}
Write-Host 'All four runtime transactions passed; Button 4 is Back. No onboard save or physical macro execution.' -ForegroundColor Green
exit 0
