param(
    [Parameter(Mandatory=$true)]
    [string]$Path
)

$ErrorActionPreference = 'Stop'
$resolvedPath = (Resolve-Path -LiteralPath $Path).Path
$manifest = [System.IO.File]::ReadAllText($resolvedPath)
if ($manifest -notmatch '(?m)^ManifestType: installer\s*$') {
    throw 'Expected a WinGet installer manifest.'
}

# WinGet requires compatible languages during upgrades when this field is set.
# Leave PackageLocale and DefaultLocale in their separate manifests untouched.
$manifest = $manifest -replace '(?m)^[ \t]*InstallerLocale:[^\r\n]*(?:\r?\n|$)', ''
# Preserve the YAML sequence item if locale is the first property of an installer.
$manifest = $manifest -replace '(?m)^([ \t]*-)[ \t]+InstallerLocale:[^\r\n]*', '$1'
[System.IO.File]::WriteAllText($resolvedPath, $manifest)
