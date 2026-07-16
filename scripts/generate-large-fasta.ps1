<#
.SYNOPSIS
    Generates a large synthetic FASTA file for SeqFlash performance testing.

.DESCRIPTION
    Produces a multi-record FASTA file of approximately the requested size, with
    random DNA sequences. Large output files are NEVER committed to git — they
    live only on disk for local benchmarking.

    Per DEVELOPMENT_PLAN.md section 26.5:
      - The default output directory is NOT on the C: drive.
      - The output directory can be overridden with -OutputDirectory.

.PARAMETER SizeGB
    Approximate target size in GiB (default: 1).

.PARAMETER OutputDirectory
    Directory to write the file into. Defaults to D:\SeqFlashTestData, or
    $env:TEMP\SeqFlashTestData if D: is not available.

.PARAMETER RecordLength
    Bases per FASTA record before wrapping at 70 columns (default: 50000).

.EXAMPLE
    .\scripts\generate-large-fasta.ps1 -SizeGB 1
    .\scripts\generate-large-fasta.ps1 -SizeGB 4 -OutputDirectory E:\bench
#>
[CmdletBinding()]
param(
    [double]$SizeGB = 1,
    [string]$OutputDirectory = "",
    [int]$RecordLength = 50000
)

$ErrorActionPreference = "Stop"

if (-not $OutputDirectory) {
    if (Test-Path "D:\") {
        $OutputDirectory = "D:\SeqFlashTestData"
    }
    else {
        $OutputDirectory = Join-Path $env:TEMP "SeqFlashTestData"
    }
}
if (-not (Test-Path $OutputDirectory)) {
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
}

$targetBytes = [int64]($SizeGB * 1GB)
$fileName = "large-{0}gb.fasta" -f [int]$SizeGB
$outPath = Join-Path $OutputDirectory $fileName

Write-Host "Generating ~$SizeGB GiB FASTA -> $outPath"

$lineWidth = 70
$bases = "ACGT"
$rng = [System.Random]::new(20260716)
$sw = [System.Diagnostics.Stopwatch]::StartNew()

# StreamWriter with a large buffer; autoflush off for speed.
$writer = [System.IO.StreamWriter]::new($outPath, $false, [System.Text.Encoding]::ASCII, 4MB)
try {
    $written = [int64]0
    $recordNum = 0
    while ($written -lt $targetBytes) {
        $recordNum++
        $writer.Write(">seq_$recordNum synthetic record`n")

        $remaining = $RecordLength
        while ($remaining -gt 0) {
            $take = [Math]::Min($lineWidth, $remaining)
            $chars = New-Object char[] $take
            for ($i = 0; $i -lt $take; $i++) {
                $chars[$i] = $bases[$rng.Next(4)]
            }
            $writer.Write($chars, 0, $take)
            $writer.Write("`n")
            $remaining -= $take
        }

        # Approximate bytes for this record: header (~30) + bases + newlines.
        $written += 30 + $RecordLength + [int]([Math]::Ceiling($RecordLength / $lineWidth))
        if ($recordNum % 2000 -eq 0) {
            Write-Host ("  records: {0}  (~{1:N0} MiB)" -f $recordNum, ($written / 1MB))
        }
    }
}
finally {
    $writer.Flush()
    $writer.Close()
}
$sw.Stop()

$actualSize = (Get-Item $outPath).Length
Write-Host ("Done: {0:N0} bytes ({1:N2} GiB), {2} records, {3:N1}s" -f `
        $actualSize, ($actualSize / 1GB), $recordNum, $sw.Elapsed.TotalSeconds)
Write-Host "File: $outPath"
