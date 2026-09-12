param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 127)]
    [int]$DeviceAddress,

    [ValidateRange(1, 300)]
    [int]$DurationSeconds = 15,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

$ErrorActionPreference = "Stop"

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
        "-DeviceAddress", $DeviceAddress,
        "-DurationSeconds", $DurationSeconds,
        "-OutputPath", "`"$OutputPath`""
    )

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

$fullOutputPath = [IO.Path]::GetFullPath($OutputPath)
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

Write-Output ("Capturing {0}, device address {1}, for {2} seconds..." -f $Interface, $DeviceAddress, $DurationSeconds)

$captureOptions = @{
    FilePath     = $usbPcap
    ArgumentList = $captureArguments
    PassThru     = $true
    NoNewWindow  = $true
}
$capture = Start-Process @captureOptions

try {
    Start-Sleep -Seconds $DurationSeconds
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
Write-Output "Capture saved: $($file.FullName) ($($file.Length) bytes)"
