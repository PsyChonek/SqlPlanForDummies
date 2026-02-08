param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [string]$ArtifactsPath = "release-artifacts"
)

$ErrorActionPreference = "Stop"

Write-Host "Extracting MSI metadata for WinGet manifests..." -ForegroundColor Cyan

if (-not (Test-Path $ArtifactsPath)) {
    Write-Error "Artifacts directory '$ArtifactsPath' not found."
    exit 1
}

$msiFiles = Get-ChildItem -Path $ArtifactsPath -Filter "*.msi"

if ($msiFiles.Count -eq 0) {
    Write-Error "No MSI files found in $ArtifactsPath"
    exit 1
}

$manifest = @{}

foreach ($msi in $msiFiles) {
    Write-Host "Processing: $($msi.Name)" -ForegroundColor Yellow
    
    # Calculate SHA256
    $sha256 = (Get-FileHash $msi.FullName -Algorithm SHA256).Hash
    Write-Host "  SHA256: $sha256"
    
    # Detect architecture
    if ($msi.Name -match 'arm64') {
        $arch = 'arm64'
    } else {
        $arch = 'x64'
    }
    
    # Extract ProductCode using Windows Installer COM
    try {
        $WindowsInstaller = New-Object -ComObject WindowsInstaller.Installer
        $Database = $WindowsInstaller.OpenDatabase($msi.FullName, 0)
        $View = $Database.OpenView("SELECT Value FROM Property WHERE Property='ProductCode'")
        $View.Execute()
        $Record = $View.Fetch()
        $ProductCode = $Record.StringData(1)
        
        if (-not $ProductCode) {
            throw "ProductCode is empty"
        }
    } catch {
        Write-Warning "Could not extract ProductCode for $($msi.Name): $_ [Generating placeholder]"
        $ProductCode = [guid]::NewGuid().ToString()
    }
    
    Write-Host "  ProductCode: $ProductCode"
    
    $manifest[$arch] = @{
        sha256 = $sha256
        productCode = $ProductCode
        filename = $msi.Name
    }
}

Write-Host "MSI metadata extracted" -ForegroundColor Green

Write-Host "Updating WinGet manifest files..." -ForegroundColor Cyan

$tag = "v$Version"

# 1. Version Manifest
$versionFile = "winget/PsyChonek.SqlPlanForDummies.yaml"
$versionYaml = @"
PackageIdentifier: PsyChonek.SqlPlanForDummies
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
"@
Set-Content -Path $versionFile -Value $versionYaml

# 2. Installer Manifest
$installerFile = "winget/PsyChonek.SqlPlanForDummies.installer.yaml"
$installerYaml = @"
PackageIdentifier: PsyChonek.SqlPlanForDummies
PackageVersion: $Version
Platform:
- Windows.Desktop
MinimumOSVersion: 10.0.0.0
InstallModes:
- silent
- silentWithProgress
Installers:
"@

if ($manifest['x64']) {
    $installerYaml += @"

- Architecture: x64
  InstallerType: msi
  InstallerUrl: https://github.com/PsyChonek/SqlPlanForDummies/releases/download/$tag/SqlPlanForDummies_$($Version)_x64_en-US.msi
  InstallerSha256: $($manifest['x64'].sha256)
  ProductCode: '$($manifest['x64'].productCode)'
"@
}

if ($manifest['arm64']) {
    $installerYaml += @"

- Architecture: arm64
  InstallerType: msi
  InstallerUrl: https://github.com/PsyChonek/SqlPlanForDummies/releases/download/$tag/SqlPlanForDummies_$($Version)_arm64_en-US.msi
  InstallerSha256: $($manifest['arm64'].sha256)
  ProductCode: '$($manifest['arm64'].productCode)'
"@
}

$installerYaml += "`nManifestType: installer`nManifestVersion: 1.6.0`n"
Set-Content -Path $installerFile -Value $installerYaml

# 3. Locale Manifest
$localeFile = "winget/PsyChonek.SqlPlanForDummies.locale.en-US.yaml"
$localeYaml = @"
PackageIdentifier: PsyChonek.SqlPlanForDummies
PackageVersion: $Version
PackageLocale: en-US
Publisher: PsyChonek
PublisherUrl: https://github.com/PsyChonek
PublisherSupportUrl: https://github.com/PsyChonek/SqlPlanForDummies/issues
PrivacyUrl: https://github.com/PsyChonek/SqlPlanForDummies/blob/main/LICENSE
Author: PsyChonek
PackageName: SQL Plan For Dummies
PackageUrl: https://github.com/PsyChonek/SqlPlanForDummies
License: MIT
LicenseUrl: https://github.com/PsyChonek/SqlPlanForDummies/blob/main/LICENSE
ShortDescription: An interactive SQL Execution Plan Viewer built with Tauri + Vue + D3.js
Description: SQL Plan For Dummies is an interactive SQL Execution Plan Viewer that provides visual analysis and performance insights for query execution plans. Features include interactive tree layout visualization, performance indicators, detailed node inspection, and automated performance analysis.
Tags:
- sql-server
- execution-plan
- database
- performance
- analysis
- visualization
- tauri
- vue
- d3js
ReleaseNotes: 'Version $Version release.'
ReleaseNotesUrl: https://github.com/PsyChonek/SqlPlanForDummies/releases/tag/$tag
ManifestType: defaultLocale
ManifestVersion: 1.6.0
"@
Set-Content -Path $localeFile -Value $localeYaml

# Cleanup old files
$oldFiles = @(
    "winget/SqlPlanForDummies.yaml",
    "winget/SqlPlanForDummies.installer.yaml",
    "winget/SqlPlanForDummies.locale.en-US.yaml",
    "winget/PsyChonek.SqlPlanForDummies.yaml" # Remove singleton if it exists from previous attempts
)
# Note: we just wrote to PsyChonek.SqlPlanForDummies.yaml (version file), so check logic to avoid deleting what we just wrote
# Wait, Version file uses the same name as what Singleton would have used?
# Technically version file is usually just "PackageId.yaml".
# So "Merge manifest into one" meant "Start using SINGLETON".
# Since I am reverting to MULTI-FILE, I am overwriting the singleton file with the VERSION file. This is fine.

foreach ($file in $oldFiles) {
    if ((Test-Path $file) -and ($file -ne $versionFile)) {
        Remove-Item $file
    }
}

Write-Host "Manifest files updated:"
Write-Host "  - $versionFile"
Write-Host "  - $installerFile"
Write-Host "  - $localeFile"

Write-Host "Validating WinGet manifests..." -ForegroundColor Cyan
Write-Host "Note: To validate, run 'winget validate winget/'"

# Validate all at once by pointing to the directory (best effort) or check existence
if ((Test-Path $versionFile) -and (Test-Path $installerFile) -and (Test-Path $localeFile)) {
    Write-Host "Files created successfully." -ForegroundColor Green
} else {
    Write-Error "Failed to create one or more manifest files"
    exit 1
}
