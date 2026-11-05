# Real-Debrid Torrent Multiple Downloader

English · [Tiếng Việt](README.vi.md)

[![Build and release](https://github.com/coolbirdzik/realdebrid/actions/workflows/build-release.yml/badge.svg)](https://github.com/coolbirdzik/realdebrid/actions/workflows/build-release.yml)

**[Download for Windows x64 (.exe)](https://github.com/coolbirdzik/realdebrid/releases/latest/download/RealDebridBatch-win-x64.exe)** · [ZIP package](https://github.com/coolbirdzik/realdebrid/releases/latest/download/RealDebridBatch-win-x64.zip) · [All releases](https://github.com/coolbirdzik/realdebrid/releases)

Browse and download selected torrent files with Real-Debrid from a Windows desktop queue. Paste magnets, direct `.torrent` URLs or choose local `.torrent` files. Browse folders, search filenames, filter videos and add only your selected results to the queue.

## Screenshot

![Real-Debrid Batch showing torrent folders and video search results](docs/images/app-en.png)

Rendered directly from the WPF interface using sample data, without a real account or torrent transfer. [View the Vietnamese interface](docs/images/app-vi.png).

<details>
<summary>Dark appearance</summary>

![Real-Debrid Batch in dark appearance](docs/images/app-en-dark.png)

</details>

## Features

| Feature | Behavior |
| --- | --- |
| Bulk input | Magnets, direct `.torrent` URLs, multiple local `.torrent` files and drag-and-drop. |
| Torrent browser | Folder tree with List/Grid views, and filename/path search through subfolders. |
| Video search and selection | Filter video formats, select individual files or all matching results, then add them to the queue. |
| Virtual library cleanup | Search and remove unwanted files, undo removals, then queue all remaining files. Exclusions and view preferences survive restart. |
| New batches | Clear the whole virtual library, input and filters with **New batch**, while keeping existing queue work. |
| Remote torrent deletion | Review selected tracked torrent IDs, then delete them from Real-Debrid. Stop linked writers first, keep local files and report failures per torrent. |
| Parallel torrents | Process 1–5 torrents with separate Real-Debrid and local progress. |
| Resumable downloads | Retry, resume or stop individual/multiple rows while keeping `.part` files. |
| Queue management | Pause/stop all, remove selected rows or clear the entire queue; restore it after restarting. |
| File layout | Flat output or torrent folders, with collision handling and no overwriting. |
| English / Vietnamese | English by default; switch immediately and remember the language choice. |
| Flat desktop UI | A persistent source input, simple dividers, List/Grid views and saved light/dark/system appearance. |
| Token storage | Optional token saving with Windows DPAPI encryption for the current account. |
| Portable executable | A single Windows x64 `.exe` with the .NET runtime included. |

## Getting started

Download `RealDebridBatch-win-x64.exe` from [GitHub Releases](https://github.com/coolbirdzik/realdebrid/releases/latest) and run it directly. For the ZIP package, extract it and run `RealDebridBatch.exe`. Requires Windows x64 and an active Real-Debrid Premium account; no separate .NET installation is needed.

From source, run `artifacts\RealDebridBatch\RealDebridBatch.exe`, or use `run.ps1` to build it if needed.

1. Get your private API token from <https://real-debrid.com/apitoken>, paste it into the app and click **Connect**. An active Premium subscription is required.
2. Choose a download folder. **Flat layout** puts all files directly in that folder; uncheck it to keep separate torrent folders and preserve their structure when possible.
3. Paste one magnet or direct `.torrent` URL per line, or choose/drop multiple local `.torrent` files. These actions only fill the input box. Click **Add to library** to inspect the batch. HTML torrent detail pages are not supported. The input stays visible on every tab and remains editable while the queue or an inspection is running. New text entered during inspection is kept for the next manual import.
4. In **Torrent files**, choose **List** or **Grid**, browse folders or search names and paths. Search includes the current folder and its subfolders; **All torrents** searches the entire library. Enable **Videos only** to exclude subtitles and other files.
5. Search for unwanted files, tick them or use **Select / clear results**, then **Remove from library** (or Delete while the file view has focus). This removes only virtual entries, including now-empty folders. Local files, remote torrents and previously created queue entries are kept. **Undo** restores the last removal in the current session; exclusions remain after restarting. Ctrl+A selects visible results and Ctrl+Z undoes a removal within the file view.
6. To download what remains after cleanup, click **Queue all remaining (N)**. This covers the entire virtual library, including files outside the current folder/search, and skips previously queued files. Alternatively, select specific results and click **Add to queue**. Changing search/folder/video filters clears selection; switching List/Grid keeps it. Previously queued files can be removed from the virtual library but are never queued twice.
7. Open **Download queue** and click **Start / Resume**. Set parallel processing from 1 to 5 torrents (default: 3). Each queue entry downloads its chosen files sequentially.

Inspection adds the source to Real-Debrid to obtain metadata, but does not select files or start local downloads. **Stop inspection** cancels waiting; **Add to library** retries using the saved remote ID. The file library, virtual exclusions, view preference and selected IDs are saved across restarts. Adding more files from the same source creates a separate queue entry and remote torrent when started, so existing downloads keep their selection.

To remove an entire folder, select it in the tree and click the trash **Remove folder** button above the tree, press Delete, or use **Remove folder from library** in its right-click menu. This removes all descendants, including files hidden by the current search/video filter, from the virtual library. Removing a torrent root also hides that root, even when its metadata is empty. **Undo** restores the folder with its previous file exclusions; local files, existing queue entries and remote tracking stay intact. Empty torrent roots disappear when their last visible file is removed, and removed folders stay absent after restart.

**Shift+click** selects a range from the last selection anchor in List/Grid and the Real-Debrid table. Repeated Shift clicks keep that anchor; **Ctrl+Shift+click** adds a range and **Ctrl+click** toggles an item. Checkboxes and grid tiles also toggle individually without Ctrl. Ranges follow the visible sorted/filtered order. Changing filters resets the anchor; switching List/Grid preserves it. App confirmation and information dialogs follow the active light/dark theme, including their title bars. Destructive confirmations default to **Cancel**.

**English is the default** when no language has been selected. Switch languages at the top right, including during a download. Saved choices are preserved when updating. The adjacent appearance selector offers **System theme / Light / Dark** and follows Windows by default. Original filenames and raw server responses are preserved.

## New batches and remote torrents

Click **New batch** in **Torrent files** to clear the virtual library, excluded-file history, pending source text and search/video filters. The source input remains visible for the next batch. Existing queue entries, local downloads and remote torrents are kept. Remote IDs remain tracked after clearing the library or queue, including preview torrents that were never queued.

Open **Real-Debrid torrents** to see every torrent in the token's Real-Debrid account, including ones added on the website. The list loads when the tab opens (or with **Reload**) and shows size, status/progress, date added, and an **In queue** tag for torrents linked to the download queue. Changing the token clears the list. Select individual rows or **Select / clear all**, click **Delete selected torrents…**, review the exact names/IDs in the table, then **Confirm deletion on Real-Debrid**. **Cancel** leaves them unchanged before deletion starts. During deletion, it cancels remaining requests; a completed remote deletion cannot be undone.

Before deleting, the app checks that the token still belongs to the account the list was loaded for; otherwise nothing is deleted. It then waits for tasks linked to each selected ID to stop, then deletes that ID. Successfully deleted (or already absent) torrents leave the list, and linked queue rows are removed from saved state. Failures stay listed with their errors and stopped queue rows, so they can be retried. Local completed/partial files are kept. Virtual metadata remains browsable; queueing those files again creates a new remote torrent when started. The account table and confirmation UI use sample data in this [review screenshot](docs/images/remote-delete-en.png).

## Queue actions

- **Retry** retries a failed torrent, reusing its Real-Debrid ID and resuming partial files where supported. Other torrents can keep running.
- **Resume** starts a queued torrent or resumes a paused/stopped torrent using its assigned paths. Completed files are skipped after checking their size.
- **Stop** cancels the selected local task and keeps partial files. The remote torrent continues on Real-Debrid.
- **Remove** stops a task and waits for it to finish before removing the row. Local files, partial files and the remote torrent are kept.
- **Pause all / Stop all** stop local queue processing and preserve progress. Use **Start / Resume** to continue; failed rows need **Retry**.
- **Clear queue** asks for confirmation, stops all tasks and waits for file writers to close before removing every queue record, including completed, failed and already-stopping rows. It also cancels a pending connection. Downloaded files, `.part` files, resume metadata and remote torrents are kept.

Row actions also apply to multiple selected rows using the buttons below the queue. Real-Debrid and local progress are shown separately.

While Real-Debrid is downloading, the queue shows the reported seed count and speed. A torrent reporting **0 seed / 0 B/s** is waiting for remote sources and may not have download links yet. The app preserves the selected file IDs and keeps accepting other torrents while waiting.

## Storage and resume

Settings and queue data are stored in `%LOCALAPPDATA%\RealDebridBatch\settings.json`. Remembered tokens are encrypted with Windows DPAPI for the current Windows account. Tokens are not placed in URLs, source code or logs.

Flat downloads use `<download folder>\<filename>`. Collisions with existing files, other torrents or partial downloads receive suffixes such as ` (2)`. Folder layout uses `<download folder>\<torrent name>_<queue ID>\...`. Selected downloads require a matching link for each selected file. Combined archives, missing selections or mismatched filenames/sizes fail before downloading the affected file, preventing unwanted files from being saved. Queues saved by earlier versions retain their all-files behavior, including archive/split links.

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

`build.ps1 -FrameworkDependent` creates a smaller app that requires the .NET 10 Desktop Runtime. Automated core checks use fake HTTP responses and never upload real torrents. Checks cover metadata inspection, nested search, video filters, selected-ID requests, cached-torrent subsets, restart/resume and rejection of combined links. WPF checks use isolated settings, exercise the browser and queue, and render both languages to `artifacts/ui-checks/`.

## CI/CD and releases

- **Push to `main` / pull request:** run core and WPF checks on Windows, build a portable executable and upload the `.exe`, ZIP, SHA-256 checksums and UI screenshots to the workflow's **Artifacts** section in [GitHub Actions](https://github.com/coolbirdzik/realdebrid/actions). Artifacts are retained for 14 days and require signing in to download.
- **Manual build:** choose **Actions → Build and release → Run workflow** to run checks and download artifacts. A manual run does not publish a Release.
- **Push a `vMAJOR.MINOR.PATCH` tag:** after all checks pass, publish a GitHub Release with generated notes, the executable, ZIP and `SHA256SUMS.txt`. Tags such as `v0.2.0-beta.1` create prereleases.
- Uses the automatic `GITHUB_TOKEN`, with `contents: write` limited to the release job. No Real-Debrid token, GitHub PAT or SSH key needs to be added to repository secrets.

To publish the next version:

```powershell
git push origin main
git tag -a v0.2.1 -m 'Release v0.2.1'
git push origin v0.2.1
```

To build release assets locally:

```powershell
.\tools\Build-Release.ps1 -Version 0.2.1
```

Output goes to `artifacts/release/`. Use `Get-FileHash .\RealDebridBatch-win-x64.exe -Algorithm SHA256` and compare it with `SHA256SUMS.txt` from the same Release. Releases are not Authenticode-signed.

The icon source is `src/RdBatch.App/Assets/AppIcon.svg`. Run `tools/Generate-Icon.ps1` on Windows to rebuild its PNG and multi-resolution ICO (16–256 px). The icon is included in the window, taskbar and published executable.

To refresh README screenshots after UI changes, run the WPF checks from the repository root, then copy `artifacts/ui-checks/browser-grid-english.png` to `docs/images/app-en.png` and `browser-grid-vietnamese.png` to `docs/images/app-vi.png`. Use `browser-english-dark.png` and `browser-vietnamese-dark.png` for dark appearance.

Official API reference: <https://api.real-debrid.com/>.
