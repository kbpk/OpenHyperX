param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\\\\\.\\USBPcap\d+$')]
    [string]$Interface,

    [Parameter(Mandatory = $true)]
    [ValidateRange(0, 65535)]
    [int]$VendorId,

    [Parameter(Mandatory = $true)]
    [ValidateRange(0, 65535)]
    [int]$ProductId,

    [Parameter(Mandatory = $true)]
    [string]$PlanPath,

    [ValidateRange(1, 300)]
    [int]$DurationSeconds = 10,

    [string]$OutputDirectory = '%TEMP%'
)

$ErrorActionPreference = 'Stop'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isAdministrator = $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)

if (-not $isAdministrator) {
    $arguments = @(
        '-NoProfile',
        '-ExecutionPolicy', 'Bypass',
        '-File', "`"$PSCommandPath`"",
        '-Interface', "`"$Interface`"",
        '-VendorId', $VendorId,
        '-ProductId', $ProductId,
        '-PlanPath', "`"$PlanPath`"",
        '-DurationSeconds', $DurationSeconds,
        '-OutputDirectory', "`"$OutputDirectory`""
    )
    $elevated = Start-Process powershell.exe -Verb RunAs -ArgumentList $arguments `
        -Wait -PassThru
    exit $elevated.ExitCode
}

$captureScript = Join-Path $PSScriptRoot 'capture-windows.ps1'
if (-not (Test-Path -LiteralPath $captureScript)) {
    throw "Capture helper was not found at $captureScript."
}

$expandedPlanPath = [Environment]::ExpandEnvironmentVariables($PlanPath)
$fullPlanPath = [IO.Path]::GetFullPath($expandedPlanPath)
if (-not (Test-Path -LiteralPath $fullPlanPath)) {
    throw "Capture plan was not found at $fullPlanPath."
}
$plan = Get-Content -LiteralPath $fullPlanPath -Raw | ConvertFrom-Json
if ($plan.name -notmatch '^[a-z0-9][a-z0-9-]*$') {
    throw 'Capture plan name must contain only lowercase letters, digits and hyphens.'
}
$steps = @($plan.steps)
if ($steps.Count -eq 0) {
    throw 'Capture plan must contain at least one step.'
}

$seenSlugs = [Collections.Generic.HashSet[string]]::new(
    [StringComparer]::OrdinalIgnoreCase
)
foreach ($step in $steps) {
    if ($step.slug -notmatch '^[a-z0-9][a-z0-9-]*$') {
        throw "Invalid capture step slug: $($step.slug)"
    }
    if (-not $seenSlugs.Add([string]$step.slug)) {
        throw "Duplicate capture step slug: $($step.slug)"
    }
    if ([string]::IsNullOrWhiteSpace([string]$step.instruction)) {
        throw "Capture step '$($step.slug)' has no instruction."
    }
}

$expandedOutputDirectory = [Environment]::ExpandEnvironmentVariables(
    $OutputDirectory
)
$fullOutputDirectory = [IO.Path]::GetFullPath($expandedOutputDirectory)
if (-not (Test-Path -LiteralPath $fullOutputDirectory -PathType Container)) {
    throw "Capture output directory does not exist: $fullOutputDirectory"
}
$seriesPrefix = [string]$plan.name
$existingSeriesFiles = @(
    Get-ChildItem -LiteralPath $fullOutputDirectory -Filter "$seriesPrefix-*"
)
if ($existingSeriesFiles.Count -ne 0) {
    throw "Refusing to overwrite an existing capture series in $fullOutputDirectory."
}

$manifest = [Collections.Generic.List[object]]::new()
$manifestPath = Join-Path $fullOutputDirectory "$seriesPrefix-manifest.json"
function Write-CaptureManifest {
    [pscustomobject]@{
        name             = [string]$plan.name
        interface        = $Interface
        vendor_id        = ('0x{0:X4}' -f $VendorId)
        product_id       = ('0x{0:X4}' -f $ProductId)
        duration_seconds = $DurationSeconds
        completed        = ($manifest.Count -eq $steps.Count)
        captures         = @($manifest)
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    & icacls.exe $manifestPath /grant "$($identity.Name):(F)" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to grant the current user access to $manifestPath."
    }
}
Write-CaptureManifest

Write-Host ''
Write-Host "Capture series: $($plan.name)"
Write-Host "One UI change is allowed in each $DurationSeconds-second capture."
Write-Host "Type q at a prompt to stop before starting the next capture."
Write-Host ''

for ($index = 0; $index -lt $steps.Count; $index++) {
    $step = $steps[$index]
    $number = $index + 1
    Write-Host "[$number/$($steps.Count)] $($step.instruction)" -ForegroundColor Cyan
    $answer = Read-Host 'Press ENTER when ready, then return to NGENUITY; type q to stop'
    if ($answer -eq 'q') {
        throw "Capture series stopped before step $number."
    }

    $fileName = '{0}-{1:D2}-{2}.pcapng' -f $seriesPrefix, $number, $step.slug
    $outputPath = Join-Path $fullOutputDirectory $fileName
    & $captureScript `
        -Interface $Interface `
        -VendorId $VendorId `
        -ProductId $ProductId `
        -DurationSeconds $DurationSeconds `
        -OutputPath $outputPath

    $file = Get-Item -LiteralPath $outputPath
    $manifest.Add([pscustomobject]@{
            index       = $number
            slug        = [string]$step.slug
            instruction = [string]$step.instruction
            file         = $file.Name
            bytes        = $file.Length
        })
    Write-CaptureManifest
    Write-Host ''
}

Write-Host "Capture series complete: $fullOutputDirectory\$seriesPrefix-*" -ForegroundColor Green
Read-Host 'Press ENTER to close this window' | Out-Null
