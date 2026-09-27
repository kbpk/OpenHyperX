param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [Parameter(Mandatory = $true, ParameterSetName = "Address")]
    [ValidateRange(1, 127)]
    [int]$DeviceAddress,

    [Parameter(Mandatory = $true, ParameterSetName = "Identity")]
    [ValidateRange(0, 65535)]
    [int]$VendorId,

    [Parameter(Mandatory = $true, ParameterSetName = "Identity")]
    [ValidateRange(0, 65535)]
    [int]$ProductId,

    [ValidateRange(1, 300)]
    [int]$DurationSeconds = 15,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    # Optional handshake for an unelevated launch wrapper. The caller owns
    # this empty sidecar; it is not an arbitrary elevated output destination.
    [string]$ReadyPath
)

$ErrorActionPreference = "Stop"

$expandedOutputPath = [Environment]::ExpandEnvironmentVariables($OutputPath)
$fullOutputPath = [IO.Path]::GetFullPath($expandedOutputPath)
if (Test-Path -LiteralPath $fullOutputPath) {
    throw "Refusing to overwrite existing capture: $fullOutputPath"
}
if ($ReadyPath) {
    $ReadyPath = [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($ReadyPath))
    if ($ReadyPath -ne "$fullOutputPath.ready.json" -or
        (Split-Path -Parent $ReadyPath) -ne [IO.Path]::GetFullPath($env:TEMP).TrimEnd('\')) {
        throw 'ReadyPath must be the exact capture .ready.json sidecar directly under %TEMP%.'
    }
    $signal = Get-Item -LiteralPath $ReadyPath
    if ($signal.PSIsContainer -or $signal.Length -ne 0 -or
        ($signal.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'ReadyPath must be a caller-created empty regular file; refusing overwrite.'
    }
}

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdministrator = $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)

if (-not $isAdministrator) {
    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`"",
        "-Interface", "`"$Interface`"",
        "-DurationSeconds", $DurationSeconds,
        "-OutputPath", "`"$OutputPath`""
    )
    if ($PSCmdlet.ParameterSetName -eq "Identity") {
        $arguments += @(
            "-VendorId", $VendorId,
            "-ProductId", $ProductId
        )
    }
    else {
        $arguments += @("-DeviceAddress", $DeviceAddress)
    }
    if ($ReadyPath) { $arguments += @('-ReadyPath', "`"$ReadyPath`"") }

    $elevationOptions = @{
        FilePath     = "powershell.exe"
        Verb         = "RunAs"
        ArgumentList = $arguments
        Wait         = $true
        PassThru     = $true
    }
    $elevated = Start-Process @elevationOptions
    exit $elevated.ExitCode
}

$usbPcap = Join-Path $env:ProgramFiles "USBPcap\USBPcapCMD.exe"
if (-not (Test-Path -LiteralPath $usbPcap)) {
    throw "USBPcapCMD.exe was not found at $usbPcap."
}

if ($PSCmdlet.ParameterSetName -eq "Identity") {
    $locator = Join-Path $PSScriptRoot "locate-usbpcap-device.ps1"
    if (-not (Test-Path -LiteralPath $locator)) {
        throw "USBPcap device locator was not found at $locator."
    }
    $location = @(
        & $locator -Interface $Interface -VendorId $VendorId `
            -ProductId $ProductId -DurationSeconds 2
    )
    $locationMatch = if ($location.Count -eq 1) {
        [regex]::Match($location[0], '^.+\|(\d+)$')
    }
    if ($location.Count -ne 1 -or -not $locationMatch.Success) {
        throw "USBPcap device locator returned an invalid result: $($location -join ', ')"
    }
    $DeviceAddress = [int]$locationMatch.Groups[1].Value
    Write-Output ("Resolved {0:X4}:{1:X4} to {2}, address {3}." -f `
        $VendorId, $ProductId, $Interface, $DeviceAddress)
}

# Recheck after elevation/discovery in case another capture created it.
if (Test-Path -LiteralPath $fullOutputPath) {
    throw "Refusing to overwrite existing capture: $fullOutputPath"
}

$outputDirectory = Split-Path -Parent $fullOutputPath
if (-not (Test-Path -LiteralPath $outputDirectory)) {
    New-Item -ItemType Directory -Path $outputDirectory | Out-Null
}

$captureArguments = @(
    "-d", $Interface,
    "--devices", $DeviceAddress,
    "--inject-descriptors",
    "-o", $fullOutputPath
)

$captureOptions = @{
    FilePath     = $usbPcap
    ArgumentList = $captureArguments
    PassThru     = $true
    NoNewWindow  = $true
}
$capture = Start-Process @captureOptions
Write-Output ("Capturing {0}, device address {1}, for {2} seconds..." -f $Interface, $DeviceAddress, $DurationSeconds)

try {
    if ($ReadyPath) {
        if ($capture.HasExited) { throw 'USBPcap exited before the ready handshake.' }
        $ready = [ordered]@{
            StartedUtc = [DateTime]::UtcNow.ToString('o')
            Interface = $Interface
            DeviceAddress = $DeviceAddress
            DurationSeconds = $DurationSeconds
        }
        $ready | ConvertTo-Json | Set-Content -LiteralPath $ReadyPath -Encoding UTF8
    }
    $exitedEarly = $capture.WaitForExit($DurationSeconds * 1000)
    if ($exitedEarly) {
        throw "USBPcap exited before the requested duration (exit code $($capture.ExitCode))."
    }
}
finally {
    if (-not $capture.HasExited) {
        Stop-Process -Id $capture.Id
        $capture.WaitForExit()
    }
}

if (-not (Test-Path -LiteralPath $fullOutputPath)) {
    throw "USBPcap did not create the capture file."
}

& icacls.exe $fullOutputPath /grant "$($identity.Name):(F)" | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Failed to grant the current user access to $fullOutputPath."
}

$file = Get-Item -LiteralPath $fullOutputPath
if ($file.Length -le 24) {
    throw "USBPcap created only an empty pcapng header ($($file.Length) bytes). Re-enumerate the capture interface and device address."
}
Write-Output "Capture saved: $($file.FullName) ($($file.Length) bytes)"
