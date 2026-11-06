$ErrorActionPreference = 'Stop'
$config = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'update.json') -Encoding UTF8 -Raw | ConvertFrom-Json
$executable = [IO.Path]::GetFullPath($config.executable)
$folder = Split-Path -LiteralPath $executable
$replacement = Join-Path $PSScriptRoot 'new.exe'
$backup = Join-Path $PSScriptRoot 'previous.exe'
$moved = $false
$canClean = $false
try {
    $running = Get-Process -Id $config.processId -ErrorAction SilentlyContinue
    if ($running -and !$running.WaitForExit(120000)) { throw 'The application did not exit in time.' }
    # Retry for antivirus scanners that briefly retain a handle after the process exits.
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        try {
            Move-Item -LiteralPath $executable -Destination $backup
            $moved = $true
            break
        } catch {
            if ($attempt -eq 59) { throw }
            Start-Sleep -Milliseconds 500
        }
    }
    Move-Item -LiteralPath $replacement -Destination $executable
    Start-Process -FilePath $executable -WorkingDirectory $folder | Out-Null
    $canClean = $true
} catch {
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $folder '.debriddock-update-error.log')
    if ($moved) {
        if (Test-Path -LiteralPath $executable) { Remove-Item -LiteralPath $executable -Force }
        Move-Item -LiteralPath $backup -Destination $executable
        $canClean = $true
        Start-Process -FilePath $executable -WorkingDirectory $folder | Out-Null
    }
    else { $canClean = $true }
} finally {
    # This is the staging folder created beside the executable; never touch portable/data.
    $stage = [IO.Path]::GetFullPath($PSScriptRoot)
    if ($canClean -and (Split-Path -LiteralPath $stage) -eq $folder -and (Split-Path $stage -Leaf).StartsWith('.debriddock-update-')) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}
