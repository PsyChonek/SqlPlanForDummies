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

# Update installer manifest
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
    # Use the actual filename if possible, otherwise strictly formatted one
    # Assuming filename matches release convention
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
  InstallerUrl: https://github.com/PsyChonek/SqlPlanForDummies/releases/download/$tag/SqlPlanForDummies_$($Version)_aarch64_en-US.msi
  InstallerSha256: $($manifest['arm64'].sha256)
  ProductCode: '$($manifest['arm64'].productCode)'
"@
}

$installerYaml += "`nManifestType: installer`nManifestVersion: 1.6.0`n"

Set-Content -Path "winget/SqlPlanForDummies.installer.yaml" -Value $installerYaml

# Update version manifest
$versionYaml = @"
PackageIdentifier: PsyChonek.SqlPlanForDummies
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
"@

Set-Content -Path "winget/SqlPlanForDummies.yaml" -Value $versionYaml

# Update locale manifest
if (Test-Path "winget/SqlPlanForDummies.locale.en-US.yaml") {
    $localeYaml = Get-Content -Path "winget/SqlPlanForDummies.locale.en-US.yaml" -Raw
    $localeYaml = $localeYaml -replace "PackageVersion: [\d.]+", "PackageVersion: $Version"
    $localeYaml = $localeYaml -replace "ReleaseNotesUrl: https://github.com/PsyChonek/SqlPlanForDummies/releases/tag/v[\d.]+", "ReleaseNotesUrl: https://github.com/PsyChonek/SqlPlanForDummies/releases/tag/$tag"
    Set-Content -Path "winget/SqlPlanForDummies.locale.en-US.yaml" -Value $localeYaml
}

Write-Host "Manifest files updated" -ForegroundColor Green

Write-Host "Validating WinGet manifests..." -ForegroundColor Cyan

$required_fields = @(
    'PackageIdentifier',
    'PackageVersion',
    'ManifestType',
    'ManifestVersion'
)

foreach ($file in @("winget/SqlPlanForDummies.yaml", "winget/SqlPlanForDummies.installer.yaml", "winget/SqlPlanForDummies.locale.en-US.yaml")) {
    if (-not (Test-Path $file)) {
        Write-Error "Manifest file not found: $file"
        exit 1
    }
    
    Write-Host "  Checking $file..."
    $content = Get-Content -Path $file -Raw
    
    foreach ($field in $required_fields) {
        if ($content -notmatch $field) {
            Write-Error "Missing required field: $field in $file"
            exit 1
        }
    }
}

Write-Host "All manifest files validated" -ForegroundColor Green
