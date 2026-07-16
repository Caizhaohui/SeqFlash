<#
.SYNOPSIS
    Generates a large synthetic FASTA file for SeqFlash performance testing.

.DESCRIPTION
    Produces a multi-record FASTA file of approximately the requested size with
    random DNA sequences. Large output files are NEVER committed to git — they
    live only on disk for local benchmarking.

    Per DEVELOPMENT_PLAN.md section 26.5:
      - The default output directory is NOT on the C: drive.
      - The output directory can be overridden with -OutputDirectory.

    Performance: builds each FASTA record as one string with StringBuilder, then
    writes the whole record in a single Write() call. This is dramatically faster
    than per-character writing (minutes, not hours, for 1 GiB).

.PARAMETER SizeGB
    Approximate target size in GiB (default: 1). Accepts fractions, e.g. 0.25.

.PARAMETER OutputDirectory
    Directory to write the file into. Defaults to D:\SeqFlashTestData, or
    $env:TEMP\SeqFlashTestData if D: is not available.

.PARAMETER RecordLength
    Bases per FASTA record, wrapped at 70 columns (default: 50000).

.EXAMPLE
    .\scripts\generate-large-fasta.ps1 -SizeGB 1
    .\scripts\generate-large-fasta.ps1 -SizeGB 0.25 -OutputDirectory E:\bench
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
# Tag the file with the size in whole MiB to avoid float-in-filename.
$sizeMiB = [int][Math]::Round($SizeGB * 1024)
$fileName = "large-{0}mib.fasta" -f $sizeMiB
$outPath = Join-Path $OutputDirectory $fileName

Write-Host "Generating ~$SizeGB GiB FASTA -> $outPath"

$lineWidth = 70
$bases = "ACGT"
$rng = [System.Random]::new(20260716)
$sw = [System.Diagnostics.Stopwatch]::StartNew()

# Pre-allocate a random DNA pool large enough for one record (RecordLength bases
# plus newlines), reused per record by overwriting in place. This avoids
# per-character RNG calls inside the inner loop dominating the cost.
$seqPool = New-Object char[] $RecordLength
$lineBuf = New-Object char[] $lineWidth

# StreamWriter with a large buffer; autoflush off for throughput.
$writer = [System.IO.StreamWriter]::new($outPath, $false, [System.Text.Encoding]::ASCII, 4MB)
try {
    $written = [int64]0
    $recordNum = 0
    while ($written -lt $targetBytes) {
        $recordNum++

        # Build the whole record (header + wrapped sequence) into one string.
        $sb = [System.Text.StringBuilder]::new($RecordLength + 64)
        [void]$sb.Append(">seq_").Append($recordNum).Append(" synthetic record`n")

        # Fill the sequence pool with random bases, then slice into 70-col lines.
        for ($i = 0; $i -lt $RecordLength; $i++) {
            $seqPool[$i] = $bases[$rng.Next(4)]
        }
        $pos = 0
        while ($pos -lt $RecordLength) {
            $take = [Math]::Min($lineWidth, $RecordLength - $pos)
            [System.Array]::Copy($seqPool, $pos, $lineBuf, 0, $take)
            [void]$sb.Append($lineBuf, 0, $take).Append("`n")
            $pos += $take
        }

        # Single Write() per record.
        $recordStr = $sb.ToString()
        $writer.Write($recordStr)
        $written += $recordStr.Length

        if ($recordNum % 5000 -eq 0) {
            Write-Host ("  records: {0}  (~{1:N0} MiB, {2:N1}s)" -f `
                    $recordNum, ($written / 1MB), $sw.Elapsed.TotalSeconds)
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
