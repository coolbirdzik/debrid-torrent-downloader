# Real-Debrid Batch

English · [Tiếng Việt](README.md)

A Windows desktop app for adding batches of magnets, `.torrent` URLs and local `.torrent` files. Choose a download folder, add sources to the queue and click **Start / Resume**. The app selects **all files**, waits for Real-Debrid, then downloads everything to your computer.

## Getting started

Run `artifacts\RealDebridBatch\RealDebridBatch.exe`, or use `run.ps1` to build it if needed. The default published app includes the .NET runtime and targets Windows x64.

1. Get your private API token from <https://real-debrid.com/apitoken>, paste it into the app and click **Connect**. An active Premium subscription is required.
2. Choose a download folder. **Flat layout** puts all files directly in that folder; uncheck it to keep separate torrent folders and preserve their structure when possible.
3. Paste one magnet or direct `.torrent` URL per line. You can also choose multiple local `.torrent` files or drop them onto the window. HTML torrent detail pages are not supported.
4. Click **Add to queue**, then **Start / Resume**. Set parallel processing from 1 to 5 torrents (default: 3). Each torrent downloads its files sequentially.

Choose **English** from the language selector at the top right. The UI, tooltips, queue states and app error messages switch immediately, including during a download. Your choice is saved. Original filenames and raw server responses are preserved.

## Queue actions

- **Retry** retries a failed torrent, reusing its Real-Debrid ID and resuming partial files where supported. Other torrents can keep running.
- **Resume** starts a queued torrent or resumes a paused/stopped torrent using its assigned paths. Completed files are skipped after checking their size.
- **Stop** cancels the selected local task and keeps partial files. The remote torrent continues on Real-Debrid.
- **Remove** stops a task and waits for it to finish before removing the row. Local files, partial files and the remote torrent are kept.
- **Pause all / Stop all** stop local queue processing and preserve progress. Use **Start / Resume** to continue; failed rows need **Retry**.
- **Clear queue** asks for confirmation, stops all tasks and waits for file writers to close before removing every queue record, including completed, failed and already-stopping rows. It also cancels a pending connection. Downloaded files, `.part` files, resume metadata and remote torrents are kept.

Row actions also apply to multiple selected rows using the buttons below the queue. Real-Debrid and local progress are shown separately.

## Storage and resume

Settings and queue data are stored in `%LOCALAPPDATA%\RealDebridBatch\settings.json`. Remembered tokens are encrypted with Windows DPAPI for the current Windows account. Tokens are not placed in URLs, source code or logs.

Flat downloads use `<download folder>\<filename>`. Collisions with existing files, other torrents or partial downloads receive suffixes such as ` (2)`. Folder layout uses `<download folder>\<torrent name>_<queue ID>\...`. Archive or split links supplied by Real-Debrid are downloaded as supplied; the app does not extract them.

Changing the folder or layout only affects torrents that have not started allocating files. Existing partial downloads keep their original paths. HTTP Range/If-Range is used when supported; if a server ignores Range, the file restarts cleanly. Completed existing files are never overwritten, and sizes are checked before renaming `.part` files.

Closing the app cancels local work and saves the queue. Reopen it and click **Start / Resume** to continue. The app does not keep downloading after closing and never automatically deletes remote torrents.

## Build and verify

Requires the .NET 10 SDK. No external NuGet packages are needed.

```powershell
dotnet build src/RdBatch.App -c Release
dotnet run --project tests/RdBatch.Tests -c Release
dotnet run --project tests/RdBatch.App.Tests -c Release
.\build.ps1
```

`build.ps1 -FrameworkDependent` creates a smaller app that requires the .NET 10 Desktop Runtime. Automated core checks use fake HTTP responses and never upload real torrents. WPF checks use isolated settings and render both languages to `artifacts/ui-checks/`.

The icon source is `src/RdBatch.App/Assets/AppIcon.svg`. Run `tools/Generate-Icon.ps1` on Windows to rebuild its PNG and multi-resolution ICO (16–256 px). The icon is included in the window, taskbar and published executable.

Official API reference: <https://api.real-debrid.com/>.
