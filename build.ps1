param(
    [switch]$FrameworkDependent,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'artifacts\RealDebridBatch'),
    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$')]
    [string]$Version = '0.2.0'
)
$ErrorActionPreference = 'Stop'
$projectPath = Join-Path $PSScriptRoot 'src\RdBatch.App\RdBatch.App.csproj'
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
$selfContained = if ($FrameworkDependent) { 'false' } else { 'true' }
dotnet publish $projectPath -c Release -r win-x64 --self-contained $selfContained "-p:Version=$Version" -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:EnableCompressionInSingleFile=true -p:DebugType=None -o $outputPath
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
Write-Host "Ready: $outputPath\RealDebridBatch.exe"
