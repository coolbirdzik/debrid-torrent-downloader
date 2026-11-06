$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$testRoot = Join-Path $workspace ('artifacts/updater-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$helper = Join-Path $workspace 'src-tauri/src/portable-update.ps1'
try {
    foreach ($scenario in @('success', 'missing-download', 'launch-failure')) {
        # Quotes, spaces and Unicode must remain literal paths throughout the helper.
        $folder = Join-Path $testRoot ($scenario + " Tiếng Việt ' app")
        $stage = Join-Path $folder '.debriddock-update-test'
        New-Item -ItemType Directory -Path $stage -Force | Out-Null
        New-Item -ItemType Directory -Path (Join-Path $folder 'data') | Out-Null
        $executable = Join-Path $folder 'Custom App.exe'
        Set-Content -LiteralPath $executable -Value 'old' -NoNewline
        Set-Content -LiteralPath (Join-Path $folder 'portable') -Value 'marker' -NoNewline
        Set-Content -LiteralPath (Join-Path $folder 'data/queue.json') -Value 'queue' -NoNewline
        if ($scenario -ne 'missing-download') {
            Set-Content -LiteralPath (Join-Path $stage 'new.exe') -Value 'new' -NoNewline
        }
        @{ executable = $executable; processId = 2147483647 } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'update.json') -Encoding UTF8
        Copy-Item -LiteralPath $helper -Destination (Join-Path $stage 'update.ps1')
        $global:UpdaterTestLaunches = 0
        function Start-Process {
            param($FilePath, $WorkingDirectory)
            if ($FilePath -ne $executable -or $WorkingDirectory -ne $folder) { throw 'Incorrect restart path' }
            $global:UpdaterTestLaunches++
            if ($scenario -eq 'launch-failure' -and $global:UpdaterTestLaunches -eq 1) { throw 'Simulated restart failure' }
        }
        & (Join-Path $stage 'update.ps1')
        $expected = if ($scenario -eq 'success') { 'new' } else { 'old' }
        if ((Get-Content -LiteralPath $executable -Raw) -ne $expected) { throw "Wrong executable after $scenario" }
        if ((Get-Content -LiteralPath (Join-Path $folder 'data/queue.json') -Raw) -ne 'queue') { throw 'Queue changed' }
        if ((Get-Content -LiteralPath (Join-Path $folder 'portable') -Raw) -ne 'marker') { throw 'Portable marker changed' }
        if (Test-Path -LiteralPath $stage) { throw 'Staging was not cleaned' }
        if ($global:UpdaterTestLaunches -lt 1) { throw 'Application was not restarted' }
        Write-Output "PASS $scenario"
    }
    Remove-Item Function:Start-Process

    # Exercise the real helper in Windows PowerShell, with a running executable locked by Windows.
    # This catches failures hidden by a mocked Start-Process or an already-exited parent PID.
    $folder = Join-Path $testRoot "real-process Tiếng Việt ' app"
    $stage = Join-Path $folder '.debriddock-update-test'
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $folder 'data') | Out-Null
    $executable = Join-Path $folder 'Custom App.exe'
    $fixture = Join-Path $workspace 'scripts/fixtures/updater-process.rs'
    & rustc --edition 2024 --cfg old_version $fixture -o $executable
    if ($LASTEXITCODE -ne 0) { throw 'Could not compile old process fixture' }
    & rustc --edition 2024 $fixture -o (Join-Path $stage 'new.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Could not compile new process fixture' }
    $oldHash = (Get-FileHash -LiteralPath $executable).Hash
    $newHash = (Get-FileHash -LiteralPath (Join-Path $stage 'new.exe')).Hash
    Set-Content -LiteralPath (Join-Path $folder 'portable') -Value 'marker' -NoNewline
    Set-Content -LiteralPath (Join-Path $folder 'data/queue.json') -Value 'queue' -NoNewline
    $oldProcess = Start-Process -FilePath $executable -WorkingDirectory $folder -WindowStyle Hidden -PassThru
    @{ executable = $executable; processId = $oldProcess.Id } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'update.json') -Encoding UTF8
    Copy-Item -LiteralPath $helper -Destination (Join-Path $stage 'update.ps1')
    $helperProcess = Start-Process -FilePath powershell.exe -ArgumentList @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"' + (Join-Path $stage 'update.ps1') + '"')) -WorkingDirectory $folder -WindowStyle Hidden -PassThru
    Start-Sleep -Milliseconds 300
    if ($helperProcess.HasExited) { throw 'Helper exited before the old application' }
    if ((Get-FileHash -LiteralPath $executable).Hash -ne $oldHash) { throw 'Replaced the app before its process exited' }
    Set-Content -LiteralPath (Join-Path $folder 'exit-old') -Value 'exit'
    if (!$oldProcess.WaitForExit(10000)) { throw 'Old process did not exit' }
    if (!$helperProcess.WaitForExit(10000)) { throw 'Portable updater did not finish' }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $started = Join-Path $folder 'started.log'
    while ([DateTime]::UtcNow -lt $deadline -and (!((Get-Content -LiteralPath $started -Raw) -match '^new'))) {
        Start-Sleep -Milliseconds 20
    }
    $lines = @(Get-Content -LiteralPath $started)
    if ($lines[0] -ne 'new') { throw 'Replacement application was not restarted' }
    $newProcess = Get-Process -Id ([int]$lines[1]) -ErrorAction Stop
    if ($newProcess.Id -eq $oldProcess.Id) { throw 'The original process was not replaced' }
    if ((Get-FileHash -LiteralPath $executable).Hash -ne $newHash) { throw 'Wrong replacement executable' }
    if ((Get-Content -LiteralPath (Join-Path $folder 'data/queue.json') -Raw) -ne 'queue') { throw 'Queue changed' }
    if ((Get-Content -LiteralPath (Join-Path $folder 'portable') -Raw) -ne 'marker') { throw 'Portable marker changed' }
    if (Test-Path -LiteralPath $stage) { throw 'Staging was not cleaned' }
    Set-Content -LiteralPath (Join-Path $folder 'exit-new') -Value 'exit'
    if (!$newProcess.WaitForExit(10000)) { throw 'Replacement process did not exit after testing' }
    Write-Output 'PASS real-process: waited for exit, replaced executable, restarted the new process, preserved portable data'
} finally {
    foreach ($process in @($oldProcess, $newProcess, $helperProcess)) {
        if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
    $resolved = [IO.Path]::GetFullPath($testRoot)
    $allowed = [IO.Path]::GetFullPath((Join-Path $workspace 'artifacts')) + [IO.Path]::DirectorySeparatorChar
    if (!$resolved.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe test cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
