<#
.SYNOPSIS
    Runs SeqFlash M8 Criterion micro-benchmarks and records machine context.

.DESCRIPTION
    Invokes `cargo bench -p seqflash-bench` (Release profile via criterion)
    and appends a short environment snapshot under docs/performance/results/.

    Large end-to-end file benches still use generate-large-fasta.ps1 separately;
    this script is for repeatable micro-benchmarks only.

.PARAMETER Filter
    Optional criterion filter, e.g. "index" or "search".

.EXAMPLE
    .\scripts\benchmark.ps1
    .\scripts\benchmark.ps1 -Filter indexing
#>
[CmdletBinding()]
param(
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$resultsDir = Join-Path $root "docs\performance\results"
New-Item -ItemType Directory -Force -Path $resultsDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$metaPath = Join-Path $resultsDir "bench-env-$stamp.txt"

$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
$ramGB = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)
$os = (Get-CimInstance Win32_OperatingSystem).Caption
$rustc = (rustc -V)

@"
SeqFlash M8 micro-benchmark environment
timestamp: $stamp
CPU: $cpu
RAM_GiB: $ramGB
OS: $os
rustc: $rustc
profile: criterion (release-like)
filter: $(if ($Filter) { $Filter } else { '(all)' })
"@ | Set-Content -Path $metaPath -Encoding UTF8

Write-Host "Wrote environment snapshot: $metaPath"
Write-Host "Running cargo bench -p seqflash-bench ..."

$benchArgs = @("bench", "-p", "seqflash-bench")
if ($Filter) {
    $benchArgs += "--"
    $benchArgs += $Filter
}
& cargo @benchArgs
if ($LASTEXITCODE -ne 0) {
    throw "cargo bench failed with exit code $LASTEXITCODE"
}

Write-Host "Criterion HTML reports are under target/criterion/ (if html_reports enabled)."
Write-Host "Record wall-clock / memory for large files separately; do not invent numbers."
