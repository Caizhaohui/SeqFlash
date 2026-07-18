<#
.SYNOPSIS
    Registers SeqFlash as the default handler for FASTA/FASTQ extensions.

.DESCRIPTION
    Writes per-user Windows Registry entries (HKCU\Software\Classes) so that
    double-clicking .fa, .fastq, etc. opens the file in SeqFlash.

    Requires administrator rights: NO (writes to HKCU only).

.PARAMETER SeqFlashPath
    Full path to SeqFlash.exe. Defaults to the current directory.

.PARAMETER Unregister
    Removes the file associations instead of adding them.

.EXAMPLE
    .\register-file-assoc.ps1 -SeqFlashPath C:\Tools\SeqFlash.exe
    .\register-file-assoc.ps1 -Unregister
#>
[CmdletBinding()]
param(
    [string]$SeqFlashPath = (Join-Path $PSScriptRoot "SeqFlash.exe"),
    [switch]$Unregister
)

$ErrorActionPreference = "Stop"

$extensions = @(".fa", ".fasta", ".fna", ".ffn", ".faa", ".frn", ".fq", ".fastq")
$progId = "SeqFlash.File.1"
$friendly = "FASTA/FASTQ Sequence File"

if ($Unregister) {
    Write-Host "Removing SeqFlash file associations..."
    foreach ($ext in $extensions) {
        $extKey = "HKCU:\Software\Classes\$ext"
        if (Test-Path $extKey) {
            $prev = (Get-ItemProperty -Path $extKey -Name "(default)" -ErrorAction SilentlyContinue).'(default)'
            if ($prev -eq $progId) {
                Remove-ItemProperty -Path $extKey -Name "(default)" -Force
                Write-Host "  $ext unregistered"
            }
        }
    }
    $pgKey = "HKCU:\Software\Classes\$progId"
    if (Test-Path $pgKey) { Remove-Item -Recurse -Force $pgKey }
    Write-Host "Done."
    return
}

if (-not (Test-Path $SeqFlashPath)) {
    Write-Error "SeqFlash.exe not found at $SeqFlashPath"
    exit 1
}

$SeqFlashPath = (Resolve-Path $SeqFlashPath).Path
Write-Host "Registering SeqFlash from: $SeqFlashPath"

# Progressive ID entry.
New-Item -Path "HKCU:\Software\Classes\$progId" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\$progId" -Name "(default)" -Value $friendly

$iconKey = "HKCU:\Software\Classes\$progId\DefaultIcon"
New-Item -Path $iconKey -Force | Out-Null
Set-ItemProperty -Path $iconKey -Name "(default)" -Value "`"$SeqFlashPath`",0"

$cmdKey = "HKCU:\Software\Classes\$progId\shell\open\command"
New-Item -Path $cmdKey -Force | Out-Null
Set-ItemProperty -Path $cmdKey -Name "(default)" -Value "`"$SeqFlashPath`" `"%1`""

# Map each extension to the ProgID.
foreach ($ext in $extensions) {
    $extKey = "HKCU:\Software\Classes\$ext"
    New-Item -Path $extKey -Force | Out-Null
    Set-ItemProperty -Path $extKey -Name "(default)" -Value $progId
    Write-Host "  $ext -> SeqFlash"
}

Write-Host "Done. Double-click .fa/.fastq files to open in SeqFlash."
Write-Host "To remove, run: .\register-file-assoc.ps1 -Unregister"
