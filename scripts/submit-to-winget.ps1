#!/usr/bin/env pwsh

<#
.SYNOPSIS
Submits SqlPlanForDummies to Windows Package Manager (winget-pkgs)

.DESCRIPTION
Automatically forks winget-pkgs (if needed), creates a branch, copies manifest files,
and submits a PR with proper commit messages.

.PARAMETER Version
The version to submit (without 'v' prefix). Defaults to latest git tag.

.PARAMETER GithubToken
GitHub personal access token with repo and workflow scopes.
Can be set via GITHUB_TOKEN environment variable.

.PARAMETER DryRun
Show what would be done without making changes.

.EXAMPLE
./submit-to-winget.ps1 -Version "1.1.0"

.EXAMPLE
./submit-to-winget.ps1 -GithubToken $token -DryRun
#>

param(
    [Parameter(Mandatory = $false)]
    [string]$Version,
    
    [Parameter(Mandatory = $false)]
    [string]$GithubToken = $env:GITHUB_TOKEN,
    
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# Get version from git tag if not provided
if (-not $Version) {
    $Version = git describe --tags --abbrev=0 | ForEach-Object { $_ -replace '^v', '' }
    if (-not $Version) {
        Write-Error "Could not determine version. Please provide -Version parameter or ensure git tags exist."
        exit 1
    }
}

Write-Host "Submitting SqlPlanForDummies v$Version to winget-pkgs" -ForegroundColor Cyan

if (-not $GithubToken) {
    Write-Error "GitHub token required. Set GITHUB_TOKEN environment variable or provide -GithubToken parameter"
    exit 1
}

# Create temporary directory for work
$tempDir = Join-Path $env:TEMP "winget-submit-$(Get-Random)"
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
    Push-Location $tempDir
    
    Write-Host "Working directory: $tempDir"
    
    # Configure git
    git config --global credential.helper store
    "https://oauth2:$GithubToken@github.com" | Set-Content -Path "$env:USERPROFILE\.git-credentials" -Force
    git config --global user.email "github-action@example.com"
    git config --global user.name "GitHub Action"
    
    # Clone winget-pkgs
    $branchName = "add-sql-plan-for-dummies-$Version"
    Write-Host "Cloning Microsoft/winget-pkgs..." -ForegroundColor Yellow
    
    if ($DryRun) {
        Write-Host "   [DRY RUN] Would clone https://github.com/microsoft/winget-pkgs"
    } else {
        git clone --depth 1 https://github.com/microsoft/winget-pkgs.git
        
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to clone winget-pkgs repository"
            exit 1
        }
    }
    
    $repoPath = Join-Path $tempDir "winget-pkgs"
    
    if (-not $DryRun) {
        Push-Location $repoPath
        
        # Create branch
        Write-Host "Creating branch: $branchName" -ForegroundColor Yellow
        git checkout -b $branchName
        
        # Create manifest directory
        $manifestDir = "manifests/p/PsyChonek/SqlPlanForDummies/$Version"

        New-Item -ItemType Directory -Path $manifestDir -Force | Out-Null
        
        # Copy manifest files from source repo
        $sourceManifestDir = Join-Path (git rev-parse --show-toplevel) "winget"
        Write-Host "Copying manifest files from: $sourceManifestDir" -ForegroundColor Yellow
        
        if (-not (Test-Path $sourceManifestDir)) {
            Write-Error "Source manifest directory not found: $sourceManifestDir"
            exit 1
        }
        
        Copy-Item -Path "$sourceManifestDir/*.yaml" -Destination $manifestDir -Force
        
        # Verify files were copied
        $copiedFiles = Get-ChildItem -Path $manifestDir -Filter "*.yaml"
        if ($copiedFiles.Count -ne 3) {
            Write-Warning "Expected 3 manifest files, found: $($copiedFiles.Count)"
        }
        
        Write-Host "Copied manifest files:"
        Get-ChildItem -Path $manifestDir | ForEach-Object { Write-Host "   - $($_.Name)" }
        
        # Add and commit
        Write-Host "Committing changes..." -ForegroundColor Yellow
        git add "manifests/p/PsyChonek/SqlPlanForDummies/$Version"
        git commit -m "Add SqlPlanForDummies version $Version

Publisher: PsyChonek
Identifier: PsyChonek.SqlPlanForDummies
Version: $Version

SQL Plan For Dummies is an interactive SQL Execution Plan Viewer built with 
Tauri + Vue + D3.js. It provides visual analysis and performance insights for 
query execution plans.

Features:
- Interactive tree layout visualization
- Performance indicators with color-coded nodes
- Detailed node inspection with comprehensive metrics
- Automated performance issue detection
- Plan comparison engine
- Export to PNG/SVG

Repository: https://github.com/PsyChonek/SqlPlanForDummies
License: MIT"
        
        # Push to fork
        $pushUrl = "https://oauth2:$GithubToken@github.com/$([System.Environment]::UserName)/winget-pkgs.git"
        Write-Host "Pushing branch..." -ForegroundColor Yellow
        git push -u origin $branchName
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Branch pushed successfully" -ForegroundColor Green
            Write-Host ""
            Write-Host "Next steps:" -ForegroundColor Green
            Write-Host "   1. Visit: https://github.com/microsoft/winget-pkgs"
            Write-Host "   2. Create a Pull Request from your fork ($branchName branch)"
            Write-Host "   3. Use the title: 'Add SqlPlanForDummies version $Version'"
            Write-Host "   4. Reference: https://github.com/PsyChonek/SqlPlanForDummies"
        } else {
            Write-Error "Failed to push branch"
            exit 1
        }
        
        Pop-Location
    } else {
        Write-Host "   [DRY RUN] Would have completed submission process"
        Write-Host "   Branch: $branchName"
        Write-Host "   Directory: manifests/p/PsyChonek/SqlPlanForDummies/$Version"
    }
    
} finally {
    Pop-Location
    
    # Cleanup
    if (-not $DryRun) {
        Write-Host ""
        Write-Host "Cleaning up temporary directory..." -ForegroundColor Gray
        Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ""
Write-Host "Done!" -ForegroundColor Green
