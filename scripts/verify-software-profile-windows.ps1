param([switch]$ConfirmRuntimeWrites)

# Opt-in test of the real executable. Changes polling only, verifies every
# profile byte, restores after success, and never saves onboard. An ambiguous
# failure stops without automatic retry or blind restoration.
$ErrorActionPreference = 'Stop'
if (-not $ConfirmRuntimeWrites) { throw 'Pass -ConfirmRuntimeWrites to authorize the volatile polling test.' }
if (@(Get-Process | Where-Object { $_.ProcessName -match 'NGenuity|OpenRGB' }).Count) {
    throw 'Close NGENUITY and OpenRGB, including their tray processes, before testing.'
}
$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
$binary = Join-Path $repo 'target\x86_64-pc-windows-msvc\debug\hyperx-cli.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Build the native Windows executable first.' }
$outputDirectory = Join-Path ([IO.Path]::GetTempPath()) ('openhyperx-profile-verify-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $outputDirectory | Out-Null
Write-Host "Polling-only test; no DPI, binding, macro, RGB or onboard changes. Logs: $outputDirectory"

function Invoke-TestCli([string]$Name, [string[]]$Arguments) {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = @(& $binary @Arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
    }
    finally { $ErrorActionPreference = $previous }
    $clean = ($output -join "`n") -replace '\x1b\[[0-9;]*m', ''
    $clean | Set-Content -LiteralPath (Join-Path $outputDirectory "$Name.log") -Encoding UTF8
    if ($code -ne 0) { throw "CLI failed at $Name ($code). STOP without retry/restoration: $clean" }
    return $clean
}

function Get-Reports([string]$Output, [string]$Direction) {
    foreach ($match in [regex]::Matches($Output, "direction=`"$Direction`" interface=1 report_id=0x07 bytes=([0-9A-F ]+)")) {
        $hex = $match.Groups[1].Value.Trim().Replace(' ', '').ToLowerInvariant()
        if ($hex.Length -ne 528) { throw 'Unexpected report length. STOP.' }
        $hex
    }
}

function Get-Snapshot([string]$Output) {
    $reads = @(Get-Reports $Output 'RX')
    if ($reads.Count -ne 1 -or -not $reads[0].StartsWith('078104')) {
        throw 'Expected one complete runtime image. STOP.'
    }
    return $reads[0]
}

$baselineOutput = Invoke-TestCli 'baseline' @('--trace', 'info')
if ($baselineOutput -notmatch 'Release: 0x1124') { throw 'Hardware verification is restricted to confirmed release 1124.' }
$baseline = Get-Snapshot $baselineOutput
$interval = $baseline.Substring(0x18 * 2, 2)
$originalHz = switch ($interval) { '01' { 1000 }; '02' { 500 }; '04' { 250 }; '08' { 125 }; default { throw 'Unknown polling interval. No write attempted.' } }
$testHz = if ($originalHz -eq 500) { 1000 } else { 500 }
$testInterval = if ($testHz -eq 500) { '02' } else { '01' }
$expected = $baseline.Substring(0, 0x18 * 2) + $testInterval + $baseline.Substring(0x19 * 2)
$testProfile = Join-Path $outputDirectory 'polling-test.toml'
$restoreProfile = Join-Path $outputDirectory 'polling-restore.toml'
foreach ($item in @(@($testProfile, $testHz), @($restoreProfile, $originalHz))) {
    [IO.File]::WriteAllText($item[0], "name = 'Polling verification'`ndevice = 'pulsefire-raid'`n[polling]`nhz = $($item[1])`n", [Text.UTF8Encoding]::new($false))
}
Invoke-TestCli 'validate' @('profile', 'validate', $testProfile) | Write-Host
$preview = Invoke-TestCli 'preview' @('--trace', 'profile', 'apply', $testProfile, '--dry-run')
if ((Get-Snapshot $preview) -ne $baseline -or $preview -notmatch "polling: $originalHz Hz -> $testHz Hz") {
    throw 'Unexpected dry-run result. No write attempted.'
}
if (@(Get-Reports $preview 'TX' | Where-Object { -not ($_.StartsWith('07030464') -or $_.StartsWith('078100')) }).Count) {
    throw 'Dry-run sent something other than the known runtime read sequence. STOP.'
}
Write-Host "Dry-run preserved all profile bytes. Testing $originalHz -> $testHz -> $originalHz Hz."

foreach ($phase in @(@('apply', $testProfile, $expected), @('restore', $restoreProfile, $baseline))) {
    $result = Invoke-TestCli $phase[0] @('--trace', 'profile', 'apply', $phase[1])
    $reads = @(Get-Reports $result 'RX')
    if ($reads.Count -ne 2 -or $reads[1] -ne $phase[2]) { throw "Unexpected $($phase[0]) readback. STOP without retry/restoration." }
    $writes = @(Get-Reports $result 'TX' | Where-Object { $_.StartsWith('070104') })
    $expectedWrite = $phase[2].Substring(0, 2) + '01' + $phase[2].Substring(4)
    if ($writes.Count -ne 1 -or $writes[0] -ne $expectedWrite) { throw "Unexpected $($phase[0]) write. STOP." }
    if (@(Get-Reports $result 'TX' | Where-Object { -not ($_.StartsWith('070104') -or $_.StartsWith('07030464') -or $_.StartsWith('078100')) }).Count) {
        throw "Unexpected $($phase[0]) command. STOP."
    }
}
$noop = Invoke-TestCli 'noop' @('--trace', 'profile', 'apply', $restoreProfile)
if ((Get-Snapshot $noop) -ne $baseline -or $noop -notmatch 'No changes:') { throw 'Unexpected no-op state. STOP.' }
if (@(Get-Reports $noop 'TX' | Where-Object { $_.StartsWith('070104') }).Count) { throw 'No-op unexpectedly wrote a profile. STOP.' }
$final = Invoke-TestCli 'final' @('--trace', 'info')
if ((Get-Snapshot $final) -ne $baseline) { throw 'Final profile differs from baseline. STOP.' }
Write-Host "PASS: validate / dry-run / apply / restore / no-op; all runtime bytes restored. Logs: $outputDirectory"
