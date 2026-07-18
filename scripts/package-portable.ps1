<#
.SYNOPSIS
    Packages SeqFlash as a portable ZIP for distribution.

.DESCRIPTION
    Copies the release binary, README, LICENSE, and the portable setup guide
    into a zip archive suitable for end users. The archive extracts to a single
    folder that runs without installation.

.PARAMETER OutputDirectory
    Where to write the ZIP file. Defaults to the current directory.

.EXAMPLE
    .\scripts\package-portable.ps1
    .\scripts\package-portable.ps1 -OutputDirectory D:\releases
#>
[CmdletBinding()]
param(
    [string]$OutputDirectory = "."
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$releaseDir = Join-Path $projectRoot "target\release"
$exePath = Join-Path $releaseDir "seqflash-app.exe"

if (-not (Test-Path $exePath)) {
    Write-Error "Release binary not found at $exePath. Run 'cargo build --release' first."
    exit 1
}

$staging = Join-Path ([System.IO.Path]::GetTempPath()) "seqflash-portable"
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Path $staging -Force | Out-Null

# Copy the portable payload.
Copy-Item $exePath (Join-Path $staging "SeqFlash.exe")
Copy-Item (Join-Path $projectRoot "README.md") $staging
Copy-Item (Join-Path $projectRoot "LICENSE") $staging
Copy-Item (Join-Path $projectRoot "USER_GUIDE.md") $staging -ErrorAction SilentlyContinue

# Create the ZIP.
$zipName = "SeqFlash-portable-x86_64.zip"
$zipPath = Join-Path $OutputDirectory $zipName
if (Test-Path $zipPath) { Remove-Item $zipPath }

Write-Host "Packaging: $zipPath"
[System.IO.Compression.ZipFile]::CreateFromDirectory($staging, $zipPath)

# Clean up.
Remove-Item -Recurse -Force $staging

$zipSize = (Get-Item $zipPath).Length
Write-Host "Done: $zipPath ($([math]::Round($zipSize / 1MB, 1)) MiB)"
