param(
    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$')]
    [string]$Version = '0.2.0'
)
$ErrorActionPreference = 'Stop'
$repoDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$publishDirectory = Join-Path $repoDirectory 'artifacts\RealDebridBatch'
$releaseDirectory = Join-Path $repoDirectory 'artifacts\release'
& (Join-Path $repoDirectory 'build.ps1') -Version $Version -OutputDirectory $publishDirectory
[IO.Directory]::CreateDirectory($releaseDirectory) | Out-Null
$exePath = Join-Path $releaseDirectory 'RealDebridBatch-win-x64.exe'
$zipPath = Join-Path $releaseDirectory 'RealDebridBatch-win-x64.zip'
Copy-Item -LiteralPath (Join-Path $publishDirectory 'RealDebridBatch.exe') -Destination $exePath -Force
Compress-Archive -LiteralPath @(
    (Join-Path $publishDirectory 'RealDebridBatch.exe'),
    (Join-Path $repoDirectory 'README.md'),
    (Join-Path $repoDirectory 'README.en.md'),
    (Join-Path $repoDirectory 'docs')
) -DestinationPath $zipPath -Force
$checksums = @($exePath, $zipPath) | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($_))"
}
[IO.File]::WriteAllText((Join-Path $releaseDirectory 'SHA256SUMS.txt'), ($checksums -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
Write-Host "Release $Version ready: $releaseDirectory"
