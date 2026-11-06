# DebridDock: all-in-one desktop app for Real-Debrid, TorBox and AllDebrid

English · [Tiếng Việt](README.vi.md)

[![Build and release](https://github.com/coolbirdzik/debrid-torrent-downloader/actions/workflows/build.yml/badge.svg)](https://github.com/coolbirdzik/debrid-torrent-downloader/actions/workflows/build.yml)
![Platforms: Windows, macOS, Linux](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Debrid services: Real-Debrid, TorBox, AllDebrid](https://img.shields.io/badge/debrid-Real--Debrid%20%7C%20TorBox%20%7C%20AllDebrid-blueviolet)

**DebridDock is a free desktop debrid client: one app to add torrents to Real-Debrid, TorBox or AllDebrid, browse
them in a virtual library, pick the exact files you want and download them to your computer through a fast,
resumable download queue.** It replaces the browser tabs, copy-pasted links and separate download manager you would
otherwise juggle for each debrid service. It runs on Windows, macOS and Linux.

![DebridDock: virtual torrent library for Real-Debrid, TorBox and AllDebrid, light appearance](docs/images/app-en.png)

## Contents

- [Why DebridDock](#why-debriddock)
- [Features](#features)
- [How it works](#how-it-works)
- [Screenshots](#screenshots)
- [Download and install](#download-and-install)
- [FAQ](#faq)
- [For developers](#for-developers)

## Why DebridDock

A debrid service downloads a torrent on its own fast servers and gives you a direct HTTPS link. Doing that by hand
for a season of episodes or a large batch means adding each magnet on the website, waiting, selecting files,
unrestricting every link and feeding the links to a download manager. DebridDock does the whole chain in one window:

- **One UI for every debrid service.** Real-Debrid, TorBox and AllDebrid share the same library, queue and controls.
  Connect each service in Settings, then use them together; each keeps its own token and account.
- **A virtual library instead of a pile of links.** Torrents are read on the service and shown as folders and files,
  like a file explorer, before anything reaches your disk.
- **Download only what you choose to your computer.** Pick single videos, whole folders or everything left. The
  service may fetch the whole torrent into its cloud; the local queue downloads your selected files.
- **A download queue you can leave running.** Downloads start as soon as you queue them, resume after a stop or a
  crash, and retry by themselves when a connection drops.

## Features

### All-in-one debrid client

- Use **Real-Debrid, TorBox and AllDebrid together**, with one saved token per service.
- Add **magnet links**, **`.torrent` URLs** or **local `.torrent` files** (paste a list, or drag and drop files onto
  the window). With several services connected, choose **one, some or all** for each import, or remember your choice.
- Duplicates are detected per service. When the same torrent file is available through several services, queue
  actions avoid downloading that file twice and prefer the cloud copy that is furthest along.

### Virtual torrent library

- Browse every torrent as **folders and files**, with back, forward and up navigation (mouse side buttons, Alt+←/→
  and Backspace work too), breadcrumbs and an **All files** flat view.
- **List or grid view**, newest added first by default, sorting by name or date added, and sortable columns
  (name, provider, size, date and status). The **Provider** filter is always available, including in an empty library.
- **Search** across folders and subfolders, and a **Videos only** filter that surfaces every video in the current
  folder tree.
- **Import progress** shows the processed count and percentage for the active pass, with ready, pending and error
  counts across all services. Stop an import, retry pending items, or see when it is waiting for an account or a
  retry. Pending imports survive app restarts and are retried while import is unpaused; use Retry to resume paused
  work.
  The **Type / Status** column shows each torrent's service percentage and progress bar, including while metadata
  is pending; imports without progress data show their waiting state. Grid view shows the same information.
- Two separate **Library menu badges** show pending torrents and new unseen files. Opening Library clears the
  unseen-file badge; the pending badge remains until metadata arrives. The same file on several services counts once
  in the new-file badge.
- Each file shows whether it is **downloaded, partly downloaded or already queued**, and keeps that mark after you
  clear the queue or restart the app.
- Remove files or folders from the library without touching your disk or the service, with undo (Ctrl+Z).
- Select ranges with **Shift+click** or **Shift+arrow keys** across Library lists/grids, Queue and Cloud files.
  Ctrl/Cmd+click toggles individual items; Shift+click on a checkbox checks or unchecks a range. Ctrl/Cmd+A selects
  all items in the current view, including rows outside the visible scroll area.
- **Clear entire library** confirms removal of all providers' library entries, including pending imports, and resets
  the input and browser filters. It keeps the download queue, local files and cloud data.

### Cloud files and TorBox account

- **Cloud files** is a separate menu page. Browse torrents on all three services and existing **Web Downloads and
  Usenet downloads on TorBox**, with provider and download-type filters.
- Items are sorted by **newest added first across providers and types**. Each row identifies the provider, account,
  type and ID, alongside size, status and date added; the header shows the item count and combined size.
- **Delete selected items** or **Delete all cloud items** in the current provider/type filter. Both show the exact
  items and accounts in a confirmation modal with a scrollable list. Deletion stays tied to those accounts even if Settings changes;
  failed items remain visible. Delete all is unavailable while loading or when part of that scope failed to load.
- Cloud deletion removes the selected items and their files from the service, and stops/removes linked queue tasks.
  Local files are kept. It is separate from clearing the virtual library.
- The **TorBox account panel**, available in Settings and Cloud files, shows the actual Free / Essential / Standard /
  Pro plan, expiry, **AirLock used / limit / remaining**, and **30-day data usage**, with a refresh button.
  Missing quota is shown as unavailable. AirLock storage and traffic usage are displayed separately.

### Torrent downloads from debrid

- **Queue selected files, or everything that is left** in the library, with one click. Downloads start right away.
  Adding selected files that are already queued shows a warning toast with the duplicate count; only new files are added.
- **Resumable downloads** (`Range` + `If-Range`): stop, pause or close the app and continue from the exact byte later.
- **Automatic retry:** a dropped connection, a timeout or an expired download link is retried with back-off and
  resumes the partial file. **Retry all** is there for anything that still fails.
- Choose the **download folder**, keep the torrent's folder structure or save everything flat, and run **1 to 5
  torrents at the same time**.
- Progress shows both sides: the service's progress and the download to your computer, with speed and file count.

### Fast, private, cross-platform

- Built with **Rust** (download engine) and **Tauri 2** (native shell), so it is small and light on memory.
- Stays responsive with **30,000+ files** in the library and **thousands of files** in a single torrent.
- Your API token is stored in the **operating system keychain** (Windows Credential Manager, macOS Keychain, Linux
  Secret Service), never in a plain settings file, and never shown in error messages.
- Windows, macOS and Linux installers, and a **portable Windows build** that keeps all data next to the app.
- Interface in English, Vietnamese, Simplified Chinese, Turkish, Spanish, German, French, Korean, Japanese, Brazilian Portuguese and Russian; light and dark appearance.
- **In-app updates** in Settings show download progress, verify the update signature, save and pause the queue, then
  install and restart. Windows portable updates preserve the existing `data` folder. Resume the queue after restart.

## How it works

1. **Connect your services.** In Settings, paste and check the API token for each service you want to use. The app
   links to each service's token page; TorBox account details appear once connected.
2. **Add torrents.** Paste magnet links or `.torrent` URLs, or drop `.torrent` files. DebridDock adds them to your
   chosen service(s) and reads their file lists. Follow import progress and retry or stop pending work as needed.
3. **Browse and pick.** Library defaults to newest added first. Filter by provider, open folders, search or filter to
   videos, then select the files or folders you want.
4. **Download.** Click **Add to queue**, choose a destination and confirm. New queue entries start immediately;
   DebridDock waits for cloud preparation, gets direct links and downloads your selected files with resume and retry.
5. **Manage cloud separately.** Open **Cloud files** to inspect or delete remote items. Clearing Library keeps those
   cloud items and your existing queue.

DebridDock never connects to torrent peers itself: your computer only downloads over HTTPS from your debrid service.

## Screenshots

![DebridDock download queue with resumable downloads, retry all and per-row controls](docs/images/queue-en.png)

![Library import progress with processed counts and pending imports, Vietnamese interface](docs/images/import-vi.png)

![Cloud files across providers and types, with TorBox plan and AirLock quota, Vietnamese interface](docs/images/cloud-vi.png)

<details>
<summary>Dark appearance and Vietnamese interface</summary>

![DebridDock virtual library in dark appearance](docs/images/app-en-dark.png)

![DebridDock in Vietnamese](docs/images/app-vi.png)

</details>

Screenshots show the application UI with sample data; no real account or transfer is involved. The import and cloud
examples above use the Vietnamese interface.

## Download and install

Get the installer for your system from the [Releases](https://github.com/coolbirdzik/debrid-torrent-downloader/releases) page:
`.msi` or `.exe` for Windows, `.dmg` for macOS, `.deb`, `.rpm` or `.AppImage` for Linux. For Windows there is also
`DebridDock-portable-win-x64.zip`: unzip it anywhere and run `DebridDock.exe`; the queue, library and settings stay in
a `data` folder next to it (the WebView2 runtime, included with Windows 11, must be installed). It can also be built
from source with `pnpm build:portable` (see [Build and test](#build-and-test)).

The Windows `-setup.exe` lets you choose the installation folder, including another drive such as
`D:\Apps\DebridDock`, and whether to install for the current user or all users. Protected folders such as
`D:\Program Files\DebridDock` need administrator permission; choose installation for all users and approve Windows'
administrator prompt. If an older setup reports **Error opening file for writing**, cancel it and run it as
administrator, or choose a folder your account can write to. Close any running installed copy before reinstalling.

After installation, use **Settings → Updates → Update and restart** when a compatible signed update is available.
Older releases without an updater package can still be downloaded from the release page.

Builds do not yet have **Windows Authenticode signing or macOS notarization**, so the system may warn on first launch.
Updater signatures verify in-app downloads separately:

- **Windows:** SmartScreen shows "Windows protected your PC". Click *More info*, then *Run anyway*.
- **macOS:** right-click the app and choose *Open*, or run `xattr -dr com.apple.quarantine "/Applications/DebridDock.app"`.
- **Linux:** the AppImage needs `chmod +x`. To remember the API token, the `.deb` and `.rpm` need a Secret Service
  keyring (GNOME Keyring or KWallet).

## FAQ

**What is a debrid service?**
A paid service such as Real-Debrid, TorBox or AllDebrid that downloads torrents (and files from file hosts) on its own
servers and gives you fast direct download links. DebridDock is a client for those services; you need your own
account and API token.

**Is DebridDock free?**
Yes. DebridDock is free and open source (MIT). You only pay for your debrid subscription.

**Does DebridDock download torrents with BitTorrent on my computer?**
No. The debrid service fetches the torrent. DebridDock only downloads the finished files over HTTPS, so no peers ever
see your IP address.

**Can I download only some files from a torrent, such as one episode?**
Yes. That is what the virtual library is for: open the torrent, select the files or folders you want and queue them.
Only those files are downloaded to your computer. TorBox and AllDebrid fetch the torrent into their cloud without
per-file selection; choosing files in DebridDock controls the local download.

**What happens if my connection drops or I close the app mid-download?**
Partial files are kept. Connection problems are retried automatically, and after a restart **Start / Resume**
continues every file from the byte where it stopped.

**Does it work with several debrid accounts?**
Yes: keep one token for each service and use Real-Debrid, TorBox and AllDebrid together. Choose the service(s) at
import, filter Library by provider, and manage remote items in Cloud files. Each queued torrent retains its original
provider and account; it is never resumed with another account.

**What is the difference between clearing Library and deleting cloud items?**
**Clear entire library** removes virtual entries across every provider and keeps the queue, local files and cloud
items. **Delete all cloud items** targets the current Cloud files filter, shows the exact items for confirmation,
then deletes them on the service and stops/removes linked queue tasks. Local files remain in both cases.

**What do the two Library badges mean?**
The clock counts torrents still waiting for a file list. The file-plus badge counts new unseen files and clears when
you open Library. Both cover all services, regardless of the current Library filter.

**What does TorBox's remaining space mean?**
It is **AirLock storage remaining**, calculated from the quota and usage returned by TorBox. The separate 30-day
usage value is traffic, not free storage. If TorBox omits its quota data, the panel shows it as unavailable.

**Where are my token and data stored?**
The token is in your system keychain when remembering tokens is enabled. The library, queue and settings are in one
`data.json` file in the app data folder, or in `data/data.json` beside the executable in a portable build.

Please download only content you have the right to download.

## For developers

DebridDock is a Rust core and download engine, a Tauri 2 shell and a Svelte 5 UI.

### Layout

| Path | What it is |
| --- | --- |
| `crates/core` | `rdbatch-core`: provider adapters, resumable downloader, queue engine, persistence. No UI or Tauri dependency, so `cargo test` runs anywhere. |
| `src-tauri` | `rdbatch-desktop`: application state, IPC commands, coalesced events, keychain access. |
| `src` | Svelte UI. `src/lib/browser.ts` holds Library browsing logic; `src/lib/remote.ts` handles cloud scopes and reviewed deletion; `TorBoxAccount.svelte` displays account usage. |
| `scripts` | Translation checks, sample data/fake provider tools, portable builds, and signed update manifest generation and tests. |
| `.github/workflows/build.yml` | Tests on all three systems, builds installers and publishes releases. |

### Build and test

Requirements: Rust (stable), Node 22+, pnpm 10. On Linux also
`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev patchelf`.

```sh
pnpm install
pnpm tauri dev            # run the app
cargo test --workspace
pnpm check && pnpm test && node scripts/check-i18n.mjs
pnpm test:updater         # updater UI logic and release manifest tests
pnpm tauri build          # installers for the current OS, in target/release/bundle
pnpm build:portable       # portable build, in dist/portable and dist/DebridDock-portable-win-x64.zip (Windows)
```

On Windows, `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-portable-update.ps1` checks portable
replacement and rollback with disposable files.

`RDBATCH_DATA_DIR=<folder>` keeps all data (queue, library, settings) in one folder, for portable use, tests and
screenshots. The portable build ships a `portable` marker file next to the executable, which does the same with a
`data` folder beside it. API tokens are never written there; they live in the operating system keychain and fall back
to memory for the session if no keychain is available.

### How it stays fast

- Progress is never sent per chunk. The engine updates the queue; one task emits the latest copy of each changed job
  about every 250 ms. A busy UI cannot slow a transfer, and a fast transfer cannot flood the UI.
- Resuming a torrent matches all of its files in one pass, so a torrent with thousands of files resumes instantly.
- `data.json` is written atomically and debounced, except when a remote torrent ID is created: that is flushed
  immediately so a crash cannot orphan a torrent on the service.
- Lists are virtualized, and the Library filters 30,000 files in a few milliseconds (`browser.test.ts` checks it).

### Safety rules the tests cover

Resume with `Range` + `If-Range`, restart when the server ignores `Range`, reject a wrong `Content-Range`, never
accept a short body as finished, never overwrite a finished file, keep partial files when stopped, reject path
traversal and symlinked folders, retry connection failures but never replay an ambiguous upload or delete, keep
tokens out of every error message (including the TorBox download URL), refuse torrents from another account or
provider.
Cloud tests also cover provider/account/type isolation for overlapping IDs, reviewed bulk deletion, and unknown
TorBox quota values.

### Releases

In **Settings → Updates**, **Update and restart** downloads and verifies the signed update, saves and pauses the
queue, installs it, and reopens the app. Windows portable copies replace their existing executable in place;
the `portable` marker and `data` folder remain intact. Resume the saved queue after restart. Older releases
without `latest.json` still have a link to their GitHub release page.

Release tags require the repository secret `TAURI_SIGNING_PRIVATE_KEY`, containing the private updater key,
and optionally `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The public key is pinned in `src-tauri/tauri.conf.json`.
The matching private key on the initial setup machine is `%USERPROFILE%\.tauri\debriddock-updater.key`;
back it up securely and never commit it. Set the GitHub Actions secret before publishing the first updater-enabled
release. Keep using the same key for subsequent releases so existing installations can verify them.

For a local signed installer build in PowerShell:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\debriddock-updater.key" -Raw
pnpm tauri build --config '{"bundle":{"createUpdaterArtifacts":true}}'
```

CI publishes the installer signatures, a signed raw executable for Windows portable copies, the signed macOS
app archive and Linux AppImage, and `latest.json`. Tag builds fail if the key or an update artifact is missing.
Updater signatures are separate from Windows Authenticode and macOS notarization.

`scripts/update-manifest.mjs` reads the release repository from the updater endpoint in `src-tauri/tauri.conf.json`
and checks it against CI's `GITHUB_REPOSITORY`. When renaming the repository, also update release discovery and the
trusted URL prefix in `src-tauri/src/update.rs`; tests check that these URLs agree.

`.github/workflows/build.yml` tests on all three systems and builds installers on every push to `main` and pull
request (uploaded as workflow artifacts for 14 days). For a tag named `vX.Y.Z` that matches the `version` in
`package.json` (the app, the installers and the update check all read it) it also publishes a GitHub release with
checksums. Replace `X.Y.Z` below with that version after updating the version files:

```sh
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

### History

DebridDock was called Real-Debrid Batch until it grew beyond one service. It began as a Windows-only WPF/.NET app;
that code was removed when the Tauri app replaced it and remains in the git history up to commit `8e3a5d1`.

## License

MIT.
