$ErrorActionPreference = 'Stop'
$exePath = Join-Path $PSScriptRoot 'artifacts\RealDebridBatch\RealDebridBatch.exe'
if (!(Test-Path -LiteralPath $exePath)) { & (Join-Path $PSScriptRoot 'build.ps1') }
Start-Process -FilePath $exePath -WindowStyle Normal
