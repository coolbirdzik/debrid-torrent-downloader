param([switch]$FrameworkDependent)
$ErrorActionPreference = 'Stop'
$projectPath = Join-Path $PSScriptRoot 'src\RdBatch.App\RdBatch.App.csproj'
$outputPath = Join-Path $PSScriptRoot 'artifacts\RealDebridBatch'
$selfContained = if ($FrameworkDependent) { 'false' } else { 'true' }
dotnet publish $projectPath -c Release -r win-x64 --self-contained $selfContained -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:EnableCompressionInSingleFile=true -p:DebugType=None -o $outputPath
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
Write-Host "Ready: $outputPath\RealDebridBatch.exe"
