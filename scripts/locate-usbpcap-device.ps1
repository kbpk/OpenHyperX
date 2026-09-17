param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [ValidateRange(0, 65535)]
    [int]$VendorId = 0x0951,

    [ValidateRange(0, 65535)]
    [int]$ProductId = 0x16E4,

    [ValidateRange(1, 5)]
    [int]$DurationSeconds = 2,

    [Parameter(DontShow = $true)]
    [string]$ResultPath
)

$ErrorActionPreference = "Stop"

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdministrator = $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)

if (-not $isAdministrator) {
    $temporaryResult = Join-Path $env:TEMP (
        "openhyperx-usbpcap-location-{0}.txt" -f [Guid]::NewGuid()
    )
    New-Item -ItemType File -Path $temporaryResult | Out-Null
    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`"",
        "-Interface", "`"$Interface`"",
        "-VendorId", $VendorId,
        "-ProductId", $ProductId,
        "-DurationSeconds", $DurationSeconds,
        "-ResultPath", "`"$temporaryResult`""
    )

    try {
        $elevated = Start-Process -FilePath "powershell.exe" -Verb RunAs `
            -ArgumentList $arguments -Wait -PassThru
        if ($elevated.ExitCode -ne 0) {
            throw "USBPcap discovery failed with exit code $($elevated.ExitCode)."
        }
        if (-not (Test-Path -LiteralPath $temporaryResult)) {
            throw "USBPcap discovery did not produce a result."
        }
        Get-Content -LiteralPath $temporaryResult
    }
    finally {
        Remove-Item -LiteralPath $temporaryResult -Force -ErrorAction SilentlyContinue
    }
    exit 0
}

$usbPcap = Join-Path $env:ProgramFiles "USBPcap\USBPcapCMD.exe"
$tshark = Join-Path $env:ProgramFiles "Wireshark\tshark.exe"
foreach ($tool in @($usbPcap, $tshark)) {
    if (-not (Test-Path -LiteralPath $tool)) {
        throw "Required executable was not found: $tool"
    }
}

$temporaryCapture = Join-Path $env:TEMP (
    "openhyperx-usbpcap-discovery-{0}.pcapng" -f [Guid]::NewGuid()
)

try {
    $capture = Start-Process -FilePath $usbPcap -ArgumentList @(
        "-d", $Interface,
        "-A",
        "--inject-descriptors",
        "-o", $temporaryCapture
    ) -PassThru -NoNewWindow

    try {
        $exitedEarly = $capture.WaitForExit($DurationSeconds * 1000)
        if ($exitedEarly) {
            throw "USBPcap exited before discovery completed (exit code $($capture.ExitCode))."
        }
    }
    finally {
        if (-not $capture.HasExited) {
            Stop-Process -Id $capture.Id
            $capture.WaitForExit()
        }
    }

    $captureFile = Get-Item -LiteralPath $temporaryCapture
    if ($captureFile.Length -le 24) {
        throw "USBPcap returned an empty descriptor capture for $Interface."
    }

    $filter = "usb.idVendor == 0x{0:X4} && usb.idProduct == 0x{1:X4}" -f `
        $VendorId, $ProductId
    $addresses = @(
        & $tshark -r $temporaryCapture -Y $filter -T fields `
            -e usb.device_address 2>$null |
            Where-Object { $_ -match '^\d+$' } |
            Sort-Object -Unique
    )
    if ($LASTEXITCODE -ne 0) {
        throw "tshark could not inspect the USBPcap descriptor capture."
    }
    if ($addresses.Count -eq 0) {
        throw ("Device {0:X4}:{1:X4} was not found on {2}." -f `
            $VendorId, $ProductId, $Interface)
    }
    if ($addresses.Count -ne 1) {
        throw ("Device {0:X4}:{1:X4} matched multiple addresses on {2}: {3}" -f `
            $VendorId, $ProductId, $Interface, ($addresses -join ', '))
    }

    $result = "{0}|{1}" -f $Interface, $addresses[0]
    if ($ResultPath) {
        Set-Content -LiteralPath $ResultPath -Value $result -Encoding ASCII
        & icacls.exe $ResultPath /grant "$($identity.Name):(F)" | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to grant the invoking user access to the discovery result."
        }
    }
    else {
        Write-Output $result
    }
}
finally {
    Remove-Item -LiteralPath $temporaryCapture -Force -ErrorAction SilentlyContinue
}
