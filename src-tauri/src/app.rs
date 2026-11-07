//! Application state: settings, library, queue, accounts, and the background tasks that keep the UI and disk in sync.
//!
//! Progress never travels per chunk: the queue's change callback only records the latest copy of each job, and one
//! task emits those copies about every 250 ms. Persistence is debounced the same way, except when the engine asks for
//! an immediate flush (a freshly created remote torrent ID).

use chrono::Utc;
use rdbatch_core::engine::{Allocator, DownloadSlots, Engine, Queue, Session};
use rdbatch_core::error::{Error, Result};
use rdbatch_core::models::{CloudKind, CloudState, Draft, Job, JobState, Msg, Persisted, Provider, RemoteRef, RemoteTorrent, Settings, TorBoxAccount, User};
use rdbatch_core::provider::{self, Debrid, Timing};
use rdbatch_core::store::Store;
use rdbatch_core::stream::{self, StreamList, StreamSource};
use rdbatch_core::{parser, search};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

const SERVICE: &str = "dev.rdbatch.desktop";
const ALL_PROVIDERS: [Provider; 3] = [Provider::RealDebrid, Provider::TorBox, Provider::AllDebrid];

#[derive(Default)]
struct Data {
    settings: Settings,
    library: Vec<Draft>,
    remote_refs: Vec<RemoteRef>,
}

/// Shared with the queue callbacks, which are created before the `App` itself.
#[derive(Default)]
struct Notifier {
    pending: Mutex<HashMap<String, Job>>,
    dirty: AtomicBool,
    /// Only byte progress changed: worth saving eventually, never urgently.
    progress_dirty: AtomicBool,
    /// Drafts changed (added, or their file list arrived): recompute marks from every job, not only changed ones.
    marks_stale: AtomicBool,
    persist: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

/// A verified account and its client. The client is reused so every caller shares one rate limiter per service.
#[derive(Clone)]
struct Account {
    token: String,
    user: User,
    api: Arc<dyn Debrid>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub provider: Provider,
    pub has_token: bool,
    pub user: Option<User>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub library: Vec<Draft>,
    pub jobs: Vec<Job>,
    pub account: AccountDto,
    /// Every service, connected or not, in a fixed order.
    pub accounts: Vec<AccountDto>,
    pub inspect: InspectStatus,
    pub warning: Option<Msg>,
    pub data_file: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Marks {
    pub provider: Provider,
    pub source_key: String,
    pub downloaded_file_ids: Vec<i32>,
    pub partial_file_ids: Vec<i32>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct InspectStatus {
    pub running: bool,
    pub done: usize,
    pub total: usize,
}

/// A change in how far a service has fetched a Library torrent.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloudUpdate {
    pub provider: Provider,
    pub source_key: String,
    pub cloud: Option<CloudState>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    pub added: usize,
    pub duplicates: usize,
    pub errors: Vec<Msg>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub provider: Option<Provider>,
    pub language: Option<String>,
    pub appearance: Option<String>,
    pub browser_view: Option<String>,
    pub browser_sort: Option<String>,
    pub output_directory: Option<String>,
    pub flatten_files: Option<bool>,
    pub concurrency: Option<u32>,
    pub independent_cloud_downloads: Option<bool>,
    pub remember_token: Option<bool>,
    pub check_updates: Option<bool>,
    pub library_providers: Option<Vec<Provider>>,
    pub remember_library_providers: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPatch {
    pub provider: Provider,
    pub source_key: String,
    pub excluded_file_ids: Vec<i32>,
    pub is_hidden: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub provider: Provider,
    pub source_key: String,
    pub file_ids: Vec<i32>,
    /// Download folder chosen for this queue entry; `None` uses the Settings folder.
    #[serde(default)]
    pub output_directory: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteList {
    pub account: String,
    pub torrents: Vec<RemoteTorrent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub id: String,
    pub ok: bool,
    pub message: Option<Msg>,
}

pub struct App {
    handle: AppHandle,
    store: Store,
    data: Mutex<Data>,
    queue: Arc<Queue>,
    notifier: Arc<Notifier>,
    tokens: Mutex<HashMap<Provider, String>>,
    accounts: Mutex<HashMap<Provider, Account>>,
    /// One download session per service; they share file-name reservations and download slots.
    sessions: tokio::sync::Mutex<HashMap<Provider, Arc<Session>>>,
    warning: Mutex<Option<Msg>>,
    api_http: reqwest::Client,
    transfer_http: reqwest::Client,
    inspect_running: AtomicBool,
    inspect_status: Mutex<InspectStatus>,
    inspect_cancel: Mutex<CancellationToken>,
    cloud_running: AtomicBool,
    timing: Timing,
}

// Debug-build-only hooks that let an end-to-end run use a fake provider server. Release builds ignore them.
#[cfg(debug_assertions)]
fn test_api_base() -> Option<String> { std::env::var("RDBATCH_API_BASE").ok() }
#[cfg(not(debug_assertions))]
fn test_api_base() -> Option<String> { None }
#[cfg(debug_assertions)]
fn test_token() -> Option<String> { std::env::var("RDBATCH_TOKEN").ok() }
#[cfg(not(debug_assertions))]
fn test_token() -> Option<String> { None }
#[cfg(debug_assertions)]
fn test_timing() -> Timing { if std::env::var_os("RDBATCH_FAST").is_some() { Timing::fast() } else { Timing::default() } }
#[cfg(not(debug_assertions))]
fn test_timing() -> Timing { Timing::default() }

fn same_key(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn entry(provider: Provider) -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, provider.label()).ok()
}

impl App {
    pub fn new(handle: AppHandle, directory: PathBuf, default_output: String) -> Arc<App> {
        let store = Store::new(&directory);
        let loaded = store.load();
        let mut data = loaded.data;
        if data.settings.output_directory.trim().is_empty() {
            data.settings.output_directory = default_output.clone();
        }
        let notifier = Arc::new(Notifier { marks_stale: AtomicBool::new(true), ..Default::default() });
        let (sink, persist) = (notifier.clone(), notifier.clone());
        let queue = Queue::new(
            data.jobs.clone(),
            move |job, durable| {
                sink.pending.lock().unwrap().insert(job.id.clone(), job);
                if durable { sink.dirty.store(true, Ordering::SeqCst) } else { sink.progress_dirty.store(true, Ordering::SeqCst) }
            },
            move || {
                if let Some(flush) = persist.persist.lock().unwrap().as_ref() {
                    flush();
                }
            },
        );
        let api_http = reqwest::Client::builder().timeout(Duration::from_secs(60)).user_agent("DebridDock").build().expect("http client");
        // Transfers can run for hours: no total timeout, only the per-read timeouts inside the downloader.
        let transfer_http = reqwest::Client::builder().connect_timeout(Duration::from_secs(30)).user_agent("DebridDock").build().expect("http client");
        let app = Arc::new(App {
            handle,
            store,
            data: Mutex::new(Data { settings: data.settings, library: data.library, remote_refs: data.remote_torrents }),
            queue,
            notifier: notifier.clone(),
            tokens: Mutex::new(HashMap::new()),
            accounts: Mutex::new(HashMap::new()),
            sessions: tokio::sync::Mutex::new(HashMap::new()),
            warning: Mutex::new(loaded.warning),
            api_http,
            transfer_http,
            inspect_running: AtomicBool::new(false),
            inspect_status: Mutex::new(InspectStatus::default()),
            inspect_cancel: Mutex::new(CancellationToken::new()),
            cloud_running: AtomicBool::new(false),
            timing: test_timing(),
        });
        let weak = Arc::downgrade(&app);
        *notifier.persist.lock().unwrap() = Some(Box::new(move || {
            if let Some(app) = weak.upgrade() {
                app.save_now();
            }
        }));
        app
    }

    // ---- events and persistence -------------------------------------------------------------------------------

    fn emit<T: Serialize + Clone>(&self, event: &str, payload: T) {
        let _ = self.handle.emit(event, payload);
    }

    pub fn status(&self, message: Msg) {
        self.emit("status", message);
    }

    fn mark_dirty(&self) {
        self.notifier.dirty.store(true, Ordering::SeqCst);
    }

    fn persisted(&self) -> Persisted {
        let (settings, library, remote_torrents) = {
            let data = self.data.lock().unwrap();
            (data.settings.clone(), data.library.clone(), data.remote_refs.clone())
        };
        Persisted { settings, jobs: self.queue.snapshot(), library, remote_torrents }
    }

    /// Send the jobs changed since the last tick, plus any new downloaded/partial marks.
    fn flush_jobs(&self) {
        let changed: Vec<Job> = {
            let mut pending = self.notifier.pending.lock().unwrap();
            pending.drain().map(|(_, job)| job).collect()
        };
        let marks = if self.notifier.marks_stale.swap(false, Ordering::SeqCst) {
            self.record_completed(&self.queue.snapshot())
        } else if changed.is_empty() {
            Vec::new()
        } else {
            self.record_completed(&changed)
        };
        if !changed.is_empty() {
            self.emit("jobs", changed);
        }
        if !marks.is_empty() {
            self.emit("marks", marks);
        }
    }

    pub fn save_now(&self) {
        self.notifier.dirty.store(false, Ordering::SeqCst);
        self.notifier.progress_dirty.store(false, Ordering::SeqCst);
        if let Err(error) = self.store.save(&self.persisted()) {
            self.status(Msg::new("Không lưu được hàng đợi: {0}", &[&error]));
        }
    }

    /// Updating can terminate the process without a window-close event. Abort if persistence fails.
    pub async fn prepare_update(&self) -> std::result::Result<(), Msg> {
        self.queue_pause().await;
        self.flush_jobs();
        self.store.save(&self.persisted()).map_err(|error| Msg::new("Không lưu được hàng đợi: {0}", &[&error]))
    }

    /// Runs forever: coalesced job events, completion marks, and debounced saves.
    pub async fn background(self: Arc<Self>) {
        let mut tick = 0u64;
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            tick += 1;
            self.flush_jobs();
            // State changes are saved within ~1.5 s; progress alone at most every 20 s (it is rebuilt from the .part files).
            let progress_due = tick % 80 == 0 && self.notifier.progress_dirty.load(Ordering::SeqCst);
            if (tick % 6 == 0 && self.notifier.dirty.load(Ordering::SeqCst)) || progress_due {
                self.notifier.dirty.store(false, Ordering::SeqCst);
                self.notifier.progress_dirty.store(false, Ordering::SeqCst);
                let snapshot = self.persisted();
                let app = self.clone();
                let result = tokio::task::spawn_blocking(move || app.store.save(&snapshot)).await;
                if !matches!(result, Ok(Ok(()))) {
                    self.mark_dirty();
                }
            }
            if tick % 60 == 0 {
                self.clone().start_inspection();
                self.clone().start_cloud_refresh();
            }
        }
    }

    /// Port of "record completed files": remember which torrent files are downloaded or partial, so the marks
    /// survive removing queue rows and restarting. Only the jobs that just changed are looked at: marks only grow.
    fn record_completed(&self, jobs: &[Job]) -> Vec<Marks> {
        let basename = |path: &str| path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_lowercase();
        let mut data = self.data.lock().unwrap();
        let mut out: Vec<Marks> = Vec::new();
        for job in jobs.iter().filter(|job| !job.source_key.trim().is_empty()) {
            for draft in data.library.iter_mut().filter(|d| d.provider == job.provider && same_key(&d.source_key, &job.source_key)) {
                let mut changed = false;
                if job.state == JobState::Completed {
                    if let Some(selected) = &job.selected_file_ids {
                        for id in selected {
                            changed |= draft.downloaded_file_ids.insert(*id);
                        }
                    }
                }
                // Old all-file queues have no file IDs: match by name (and size), built once per draft, not per file.
                let mut by_name: Option<HashMap<String, Vec<(i32, u64)>>> = None;
                let mut ids = Vec::new();
                for file in job.files.iter().filter(|f| f.completed || f.downloaded_bytes > 0) {
                    let id = file.torrent_file_id.or_else(|| {
                        let index = by_name.get_or_insert_with(|| {
                            let mut map: HashMap<String, Vec<(i32, u64)>> = HashMap::new();
                            for c in &draft.files {
                                map.entry(basename(&c.path)).or_default().push((c.id, c.bytes));
                            }
                            map
                        });
                        let matches: Vec<i32> = index
                            .get(&basename(&file.relative_path))
                            .map(|list| list.iter().filter(|(_, bytes)| file.expected_bytes == 0 || *bytes == file.expected_bytes).map(|(id, _)| *id).collect())
                            .unwrap_or_default();
                        if matches.len() == 1 { Some(matches[0]) } else { None }
                    });
                    ids.push((id, file.completed));
                }
                for (id, completed) in ids {
                    if let Some(id) = id {
                        if completed {
                            changed |= draft.downloaded_file_ids.insert(id);
                        } else {
                            changed |= draft.partial_file_ids.insert(id);
                        }
                    }
                }
                let before = draft.partial_file_ids.len();
                let downloaded = draft.downloaded_file_ids.clone();
                draft.partial_file_ids.retain(|id| !downloaded.contains(id));
                changed |= draft.partial_file_ids.len() != before;
                if changed {
                    out.retain(|m| !(m.provider == draft.provider && same_key(&m.source_key, &draft.source_key)));
                    out.push(Marks {
                        provider: draft.provider,
                        source_key: draft.source_key.clone(),
                        downloaded_file_ids: draft.downloaded_file_ids.iter().copied().collect(),
                        partial_file_ids: draft.partial_file_ids.iter().copied().collect(),
                    });
                }
            }
        }
        if !out.is_empty() {
            self.mark_dirty();
        }
        out
    }

    // ---- snapshot, settings and tokens -------------------------------------------------------------------------

    fn provider(&self) -> Provider {
        self.data.lock().unwrap().settings.provider
    }

    pub fn snapshot(&self) -> Snapshot {
        let (settings, library) = {
            let data = self.data.lock().unwrap();
            (data.settings.clone(), data.library.clone())
        };
        let accounts: Vec<AccountDto> = ALL_PROVIDERS.iter().map(|p| self.account_dto(*p)).collect();
        let account = accounts.iter().find(|a| a.provider == settings.provider).cloned().unwrap_or_else(|| self.account_dto(settings.provider));
        Snapshot {
            account,
            accounts,
            settings,
            library,
            jobs: self.queue.snapshot(),
            inspect: self.inspect_status.lock().unwrap().clone(),
            warning: self.warning.lock().unwrap().clone(),
            data_file: self.store.path().display().to_string(),
        }
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Settings {
        let (settings, forget_tokens) = {
            let mut data = self.data.lock().unwrap();
            let s = &mut data.settings;
            let remembered = s.remember_token;
            if let Some(v) = patch.provider { s.provider = v }
            if let Some(v) = patch.language { s.language = rdbatch_core::models::normalize_language(&v) }
            if let Some(v) = patch.appearance { s.appearance = if matches!(v.as_str(), "light" | "dark") { v } else { "system".into() } }
            if let Some(v) = patch.browser_view { s.browser_view = if v == "grid" { "grid".into() } else { "list".into() } }
            if let Some(v) = patch.browser_sort { s.browser_sort = if matches!(v.as_str(), "name" | "added-desc" | "added-asc") { v } else { "added-desc".into() } }
            if let Some(v) = patch.output_directory { s.output_directory = v.trim().to_string() }
            if let Some(v) = patch.flatten_files { s.flatten_files = v }
            if let Some(v) = patch.concurrency { s.concurrency = v.clamp(1, 5) }
            if let Some(v) = patch.independent_cloud_downloads { s.independent_cloud_downloads = v }
            if let Some(v) = patch.remember_token { s.remember_token = v }
            if let Some(v) = patch.check_updates { s.check_updates = v }
            if let Some(list) = patch.library_providers {
                let mut unique = Vec::new();
                for provider in list {
                    if !unique.contains(&provider) {
                        unique.push(provider);
                    }
                }
                s.library_providers = unique;
            }
            if let Some(v) = patch.remember_library_providers { s.remember_library_providers = v }
            (s.clone(), remembered && !s.remember_token)
        };
        if forget_tokens {
            for provider in ALL_PROVIDERS {
                if let Some(e) = entry(provider) {
                    let _ = e.delete_credential();
                }
            }
        }
        self.mark_dirty();
        self.paint_title_bar();
        settings
    }

    /// Color the native title bar for the saved appearance (also called at startup and when the OS theme changes).
    pub fn paint_title_bar(&self) {
        use tauri::Manager;
        let appearance = self.data.lock().unwrap().settings.appearance.clone();
        if let Some(window) = self.handle.get_webview_window("main") {
            crate::theme::apply(&window, &appearance);
        }
    }

    fn token_for(&self, provider: Provider) -> Option<String> {
        // The debug hook stands for one account only, so a test run never sees three services on one fake server.
        if let Some(token) = test_token() {
            return (provider == self.provider()).then_some(token);
        }
        if let Some(token) = self.tokens.lock().unwrap().get(&provider) {
            return Some(token.clone());
        }
        let remember = self.data.lock().unwrap().settings.remember_token;
        if !remember {
            return None;
        }
        let token = entry(provider)?.get_password().ok().filter(|t| !t.trim().is_empty())?;
        self.tokens.lock().unwrap().insert(provider, token.clone());
        Some(token)
    }

    fn account_dto(&self, provider: Provider) -> AccountDto {
        let has_token = self.token_for(provider).is_some();
        let user = if has_token { self.accounts.lock().unwrap().get(&provider).map(|a| a.user.clone()) } else { None };
        AccountDto { provider, has_token, user }
    }

    /// Services with a token, in the fixed order.
    fn providers_with_tokens(&self) -> Vec<Provider> {
        ALL_PROVIDERS.into_iter().filter(|p| self.token_for(*p).is_some()).collect()
    }

    /// Store the token for the current provider. A keychain failure is reported but never loses the token for this
    /// session.
    pub fn save_token(&self, token: &str) -> Option<Msg> {
        let provider = self.provider();
        let token = token.trim().to_string();
        self.accounts.lock().unwrap().remove(&provider);
        if token.is_empty() {
            self.tokens.lock().unwrap().remove(&provider);
            if let Some(e) = entry(provider) {
                let _ = e.delete_credential();
            }
            return None;
        }
        self.tokens.lock().unwrap().insert(provider, token.clone());
        if !self.data.lock().unwrap().settings.remember_token {
            return None;
        }
        match entry(provider).map(|e| e.set_password(&token)) {
            Some(Ok(())) => None,
            _ => Some(Msg::plain("Không lưu được token vào kho khóa của hệ điều hành; token chỉ được giữ trong phiên này.")),
        }
    }

    fn client(&self, provider: Provider, token: &str) -> Arc<dyn Debrid> {
        provider::create(provider, self.api_http.clone(), token, test_api_base().as_deref(), self.timing)
    }

    fn missing_token(provider: Provider) -> Error {
        Error::operation("Dán API token của {0} trong Cài đặt trước khi dùng dịch vụ này.", &[&provider.label()])
    }

    /// Verify a specified service, or the one shown in Settings when omitted.
    pub async fn connect(&self, provider: Option<Provider>) -> Result<User> {
        Ok(self.connect_provider(provider.unwrap_or_else(|| self.provider())).await?.user)
    }

    /// Verify a service's token and remember the account it belongs to.
    async fn connect_provider(&self, provider: Provider) -> Result<Account> {
        let token = self.token_for(provider).ok_or_else(|| Self::missing_token(provider))?;
        let api = self.client(provider, &token);
        let user = api.user().await?;
        let account = Account { token, user: user.clone(), api };
        // Changing a token while its request is in flight must not restore the previous account in the UI/cache.
        if self.token_for(provider).as_deref() == Some(account.token.as_str()) {
            self.accounts.lock().unwrap().insert(provider, account.clone());
            // Starting the queue or reading torrents verifies tokens too; tell the UI which account is connected.
            self.emit("account", AccountDto { provider, has_token: true, user: Some(user) });
        }
        Ok(account)
    }

    async fn verified(&self, provider: Provider) -> Result<(Account, Arc<dyn Debrid>)> {
        let token = self.token_for(provider).ok_or_else(|| Self::missing_token(provider))?;
        let cached = self.accounts.lock().unwrap().get(&provider).cloned().filter(|a| a.token == token);
        let account = match cached {
            Some(account) => account,
            None => self.connect_provider(provider).await?,
        };
        let api = account.api.clone();
        Ok((account, api))
    }

    // ---- library -----------------------------------------------------------------------------------------------

    /// Add every source to each chosen service. A torrent counts as a duplicate only when every chosen service
    /// already has it.
    pub fn library_add(self: &Arc<Self>, text: &str, providers: Vec<Provider>) -> AddResult {
        let parsed = parser::parse(text);
        let mut chosen: Vec<Provider> = Vec::new();
        for provider in providers {
            if !chosen.contains(&provider) {
                chosen.push(provider);
            }
        }
        if chosen.is_empty() {
            chosen.push(self.provider());
        }
        let (mut added, mut duplicates) = (0, 0);
        let mut fresh: Vec<Draft> = Vec::new();
        {
            let mut data = self.data.lock().unwrap();
            data.settings.library_import_paused = false;
            let now = Utc::now();
            for source in parsed.sources {
                let mut known = true;
                for &provider in &chosen {
                    if data.library.iter().any(|d| d.provider == provider && same_key(&d.source_key, &source.key)) {
                        continue;
                    }
                    known = false;
                    let draft = Draft { added_at: Some(now), provider, source: source.source.clone(), source_key: source.key.clone(), name: source.name.clone(), ..Default::default() };
                    data.library.push(draft.clone());
                    fresh.push(draft);
                }
                if known { duplicates += 1 } else { added += 1 }
            }
        }
        for draft in fresh {
            self.emit("draft", draft);
        }
        self.notifier.marks_stale.store(true, Ordering::SeqCst);
        self.mark_dirty();
        self.clone().start_inspection();
        AddResult { added, duplicates, errors: parsed.errors }
    }

    pub fn library_update(&self, patches: Vec<LibraryPatch>) {
        let mut changed = Vec::new();
        {
            let mut data = self.data.lock().unwrap();
            for patch in patches {
                if let Some(draft) = data.library.iter_mut().find(|d| d.provider == patch.provider && same_key(&d.source_key, &patch.source_key)) {
                    draft.excluded_file_ids = patch.excluded_file_ids.into_iter().collect();
                    draft.is_hidden = patch.is_hidden;
                    changed.push(draft.clone());
                }
            }
        }
        for draft in changed {
            self.emit("draft", draft);
        }
        self.mark_dirty();
    }

    /// "New batch": forget the virtual library but keep queue work, local files and remote torrents.
    pub fn library_reset(&self) {
        self.inspect_cancel.lock().unwrap().cancel();
        self.data.lock().unwrap().library.clear();
        self.report_inspect(false, 0, 0);
        self.mark_dirty();
        self.save_now();
    }

    /// Retry now: forget the back-off on every uninspected torrent and resume a paused import.
    pub fn library_retry(self: &Arc<Self>) {
        {
            let mut data = self.data.lock().unwrap();
            data.settings.library_import_paused = false;
            for draft in data.library.iter_mut().filter(|d| d.files.is_empty()) {
                draft.next_inspection_at = None;
            }
        }
        self.mark_dirty();
        self.clone().start_inspection();
    }

    pub fn library_cancel_inspect(&self) {
        self.data.lock().unwrap().settings.library_import_paused = true;
        self.inspect_cancel.lock().unwrap().cancel();
        self.mark_dirty();
    }

    pub fn start_inspection(self: Arc<Self>) {
        if self.data.lock().unwrap().settings.library_import_paused || self.inspect_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let total = self.pending_drafts().len();
        if total == 0 {
            self.inspect_running.store(false, Ordering::SeqCst);
            return;
        }
        self.report_inspect(true, 0, total);
        tauri::async_runtime::spawn(async move {
            self.run_inspection().await;
            let status = self.inspect_status.lock().unwrap().clone();
            self.report_inspect(false, status.done, status.total);
            self.inspect_running.store(false, Ordering::SeqCst);
        });
    }

    fn report_inspect(&self, running: bool, done: usize, total: usize) {
        let status = InspectStatus { running, done, total };
        *self.inspect_status.lock().unwrap() = status.clone();
        self.emit("inspect", status);
    }

    /// Unread torrents of every service that has a token (read before the data lock: `token_for` takes it too).
    fn pending_drafts(&self) -> Vec<(Provider, String)> {
        let usable = self.providers_with_tokens();
        let data = self.data.lock().unwrap();
        let now = Utc::now();
        data.library
            .iter()
            .filter(|d| usable.contains(&d.provider) && d.files.is_empty() && !d.is_hidden && d.next_inspection_at.map(|t| t <= now).unwrap_or(true))
            .map(|d| (d.provider, d.source_key.clone()))
            .collect()
    }

    fn with_draft<R>(&self, provider: Provider, key: &str, change: impl FnOnce(&mut Draft) -> R) -> Option<R> {
        let mut data = self.data.lock().unwrap();
        let draft = data.library.iter_mut().find(|d| d.provider == provider && same_key(&d.source_key, key))?;
        let result = change(draft);
        let snapshot = draft.clone();
        drop(data);
        self.emit("draft", snapshot);
        self.mark_dirty();
        Some(result)
    }

    async fn run_inspection(&self) {
        let cancel = {
            let mut guard = self.inspect_cancel.lock().unwrap();
            if guard.is_cancelled() {
                *guard = CancellationToken::new();
            }
            guard.clone()
        };
        loop {
            let pending = self.pending_drafts();
            if pending.is_empty() || cancel.is_cancelled() {
                return;
            }
            let total = pending.len();
            let mut done = 0;
            self.report_inspect(true, done, total);
            for provider in ALL_PROVIDERS {
                let keys: Vec<String> = pending.iter().filter(|(p, _)| *p == provider).map(|(_, k)| k.clone()).collect();
                if keys.is_empty() {
                    continue;
                }
                if cancel.is_cancelled() {
                    return;
                }
                match self.verified(provider).await {
                    Ok((account, api)) => {
                        let engine = Engine::new(api, self.transfer_http.clone(), Some(account.user.username.clone()), self.queue.clone(), self.timing);
                        for key in keys {
                            if cancel.is_cancelled() {
                                return;
                            }
                            if !self.inspect_one(&engine, provider, &key, &account.user.username, &cancel).await {
                                return;
                            }
                            if cancel.is_cancelled() { return; }
                            done += 1;
                            self.report_inspect(true, done, total);
                        }
                    }
                    Err(error) => {
                        // Back off like a failed read, so one bad token never stalls the other services.
                        if cancel.is_cancelled() { return; }
                        let message = error.friendly();
                        for key in &keys {
                            self.with_draft(provider, key, |d| back_off(d, message.clone()));
                        }
                        done += keys.len();
                        self.report_inspect(true, done, total);
                    }
                }
            }
            self.save_now();
        }
    }

    /// Add one torrent to its service (once) and wait for its file list. Returns false when cancelled.
    async fn inspect_one(&self, engine: &Engine, provider: Provider, key: &str, username: &str, cancel: &CancellationToken) -> bool {
        let existing = self.with_draft(provider, key, |d| {
            d.inspection_error = None;
            d.next_inspection_at = None;
            d.cloud = None;
            (d.source.clone(), d.torrent_id.clone())
        }).unwrap_or_default();
        let outcome = async {
            let torrent_id = match existing.1 {
                Some(id) => id,
                None => {
                    let id = engine.add_source(&existing.0).await?;
                    let name = self.with_draft(provider, key, |d| {
                        d.torrent_id = Some(id.clone());
                        d.remote_account = Some(username.to_string());
                        d.name.clone()
                    });
                    self.data.lock().unwrap().remote_refs.push(RemoteRef {
                        provider,
                        id: id.clone(),
                        source_key: key.to_string(),
                        name: name.unwrap_or_default(),
                        account: Some(username.to_string()),
                    });
                    // Persist the remote ID before any further request; retries reuse it.
                    self.save_now();
                    id
                }
            };
            engine.inspect_with_progress(&torrent_id, cancel, |info| {
                if cancel.is_cancelled() { return; }
                let cloud = CloudState { status: info.status.clone(), progress: info.progress, speed: info.speed, seeders: info.seeders };
                let changed = {
                    let mut data = self.data.lock().unwrap();
                    // A reset or a new import can replace this source while its request is in flight.
                    let Some(draft) = data.library.iter_mut().find(|d| d.provider == provider && same_key(&d.source_key, key) && d.torrent_id.as_deref() == Some(torrent_id.as_str())) else { return; };
                    if draft.cloud.as_ref() == Some(&cloud) { false } else {
                        draft.cloud = Some(cloud.clone());
                        true
                    }
                };
                if changed {
                    self.notifier.progress_dirty.store(true, Ordering::SeqCst);
                    self.emit("cloud", vec![CloudUpdate { provider, source_key: key.to_string(), cloud: Some(cloud) }]);
                }
            }).await
        }
        .await;
        match outcome {
            Ok(info) => {
                self.with_draft(provider, key, |d| {
                    if !info.filename.trim().is_empty() {
                        d.name = info.filename.clone();
                    }
                    d.files = info.files.clone();
                    d.inspection_error = None;
                    d.inspection_attempts = 0;
                    d.next_inspection_at = None;
                    d.cloud = Some(CloudState { status: info.status.clone(), progress: info.progress, speed: info.speed, seeders: info.seeders });
                });
                self.notifier.marks_stale.store(true, Ordering::SeqCst);
            }
            Err(Error::Cancelled) => return false,
            Err(error) => {
                self.with_draft(provider, key, |d| back_off(d, error.friendly()));
            }
        }
        true
    }

    // ---- cloud progress ----------------------------------------------------------------------------------------

    pub fn start_cloud_refresh(self: Arc<Self>) {
        if self.cloud_running.swap(true, Ordering::SeqCst) {
            return;
        }
        tauri::async_runtime::spawn(async move {
            self.refresh_cloud().await;
            self.cloud_running.store(false, Ordering::SeqCst);
        });
    }

    /// A read torrent whose service copy may still change.
    fn needs_cloud(draft: &Draft) -> bool {
        !draft.is_hidden && draft.torrent_id.is_some() && !draft.files.is_empty() && draft.cloud.as_ref().map(|c| !c.is_settled()).unwrap_or(true)
    }

    /// One list request per service that still has torrents on their way to the cloud. Changes travel on their own
    /// `cloud` event, so progress never rebuilds the Library rows.
    async fn refresh_cloud(&self) {
        let usable = self.providers_with_tokens();
        let wanted: Vec<Provider> = {
            let data = self.data.lock().unwrap();
            usable.into_iter().filter(|p| data.library.iter().any(|d| d.provider == *p && Self::needs_cloud(d))).collect()
        };
        for provider in wanted {
            let Ok((_, api)) = self.verified(provider).await else { continue };
            let Ok(list) = api.list().await else { continue };
            let by_id: HashMap<&str, &RemoteTorrent> = list.iter().map(|t| (t.id.as_str(), t)).collect();
            let (updates, forgotten) = {
                let mut data = self.data.lock().unwrap();
                let (mut updates, mut forgotten) = (Vec::new(), Vec::new());
                for draft in data.library.iter_mut().filter(|d| d.provider == provider && Self::needs_cloud(d)) {
                    let id = draft.torrent_id.clone().unwrap_or_default();
                    let next = match by_id.get(id.as_str()) {
                        Some(t) => CloudState { status: t.status.clone(), progress: t.progress, speed: t.speed, seeders: t.seeders },
                        None => CloudState { status: "missing".into(), ..Default::default() },
                    };
                    if next.status == "missing" {
                        // Deleted on the service: the next queue adds the torrent again instead of using a dead ID.
                        draft.torrent_id = None;
                        draft.remote_account = None;
                        draft.cloud = Some(next);
                        forgotten.push(draft.clone());
                    } else if draft.cloud.as_ref() != Some(&next) {
                        draft.cloud = Some(next.clone());
                        updates.push(CloudUpdate { provider, source_key: draft.source_key.clone(), cloud: Some(next) });
                    }
                }
                (updates, forgotten)
            };
            if !forgotten.is_empty() {
                self.mark_dirty();
                for draft in forgotten {
                    self.emit("draft", draft);
                }
            }
            if !updates.is_empty() {
                self.notifier.progress_dirty.store(true, Ordering::SeqCst);
                self.emit("cloud", updates);
            }
        }
    }

    // ---- queue -------------------------------------------------------------------------------------------------

    /// Returns the IDs of the new jobs, so the UI can start exactly those.
    pub fn queue_add(&self, items: Vec<QueueItem>) -> Result<Vec<String>> {
        // Validate every folder first so a bad path adds nothing, instead of failing later when the job starts.
        let mut targets = Vec::with_capacity(items.len());
        for item in &items {
            let dir = item.output_directory.as_deref().map(str::trim).filter(|dir| !dir.is_empty());
            if dir.is_some_and(|dir| !std::path::Path::new(dir).is_absolute()) {
                return Err(Error::Operation(Msg::plain("Chọn một đường dẫn lưu đầy đủ.")));
            }
            targets.push(dir.map(str::to_string));
        }
        let mut created = Vec::new();
        let mut touched = Vec::new();
        {
            let mut data = self.data.lock().unwrap();
            let jobs = self.queue.snapshot();
            for (item, target) in items.into_iter().zip(targets) {
                let Some(draft) = data.library.iter_mut().find(|d| d.provider == item.provider && same_key(&d.source_key, &item.source_key)) else { continue };
                let queued = |id: i32| jobs.iter().any(|j| j.provider == draft.provider && same_key(&j.source_key, &draft.source_key) && j.selected_file_ids.as_ref().map(|s| s.contains(&id)).unwrap_or(true));
                let mut seen = HashSet::new();
                let files: Vec<_> = item.file_ids.iter().filter(|id| seen.insert(**id)).filter_map(|id| draft.files.iter().find(|f| f.id == *id)).filter(|f| !queued(f.id)).cloned().collect();
                if files.is_empty() {
                    continue;
                }
                let mut job = Job::new(draft.provider, &draft.source, &draft.source_key, &draft.name);
                job.torrent_id = draft.torrent_id.clone();
                job.remote_account = if draft.torrent_id.is_none() { None } else { draft.remote_account.clone() };
                job.selected_file_ids = Some(files.iter().map(|f| f.id).collect());
                job.target_directory = target.clone();
                job.total_bytes = files.iter().map(|f| f.bytes).sum();
                let names = files.iter().map(|f| f.path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_string()).collect::<Vec<_>>().join(", ");
                job.detail = Msg::new("Đã chọn {0} file · {1}", &[&files.len(), &names]);
                created.push(job.id.clone());
                self.queue.push(job);
                // A later selection needs its own remote torrent; an active selection cannot be changed. The job now
                // owns the remote torrent, so its own progress is what the Library shows from here on.
                draft.torrent_id = None;
                draft.remote_account = None;
                draft.cloud = None;
                touched.push(draft.clone());
            }
        }
        for draft in touched {
            self.emit("draft", draft);
        }
        self.mark_dirty();
        Ok(created)
    }

    fn output_root(&self) -> Result<(PathBuf, usize, bool, bool)> {
        let (dir, concurrency, flatten, independent) = {
            let data = self.data.lock().unwrap();
            (data.settings.output_directory.trim().to_string(), data.settings.concurrency as usize, data.settings.flatten_files, data.settings.independent_cloud_downloads)
        };
        let path = PathBuf::from(&dir);
        if dir.is_empty() || !path.is_absolute() {
            return Err(Error::Operation(Msg::plain("Chọn một đường dẫn lưu đầy đủ.")));
        }
        std::fs::create_dir_all(&path)?;
        let probe = path.join(format!(".rd-write-test-{}", uuid_like()));
        std::fs::write(&probe, b"")?;
        let _ = std::fs::remove_file(&probe);
        Ok((path, concurrency, flatten, independent))
    }

    /// Start jobs of every service. A service that cannot start (no token, no download rights) does not hold back the
    /// others; its error is reported, and returned only when nothing could start.
    pub async fn queue_start(self: &Arc<Self>, ids: Option<Vec<String>>) -> Result<()> {
        let jobs: Vec<Job> = self
            .queue
            .snapshot()
            .into_iter()
            .filter(|j| ids.as_ref().map(|ids| ids.contains(&j.id)).unwrap_or(j.can_resume()) && (j.can_resume() || j.can_retry()))
            .collect();
        if jobs.is_empty() {
            self.status(Msg::plain("Thêm torrent hoặc bấm Retry ở dòng bị lỗi."));
            return Ok(());
        }
        let mut started: Vec<Arc<Session>> = Vec::new();
        let mut failure: Option<Error> = None;
        for provider in ALL_PROVIDERS {
            let mine: Vec<&Job> = jobs.iter().filter(|j| j.provider == provider).collect();
            if mine.is_empty() {
                continue;
            }
            match self.ensure_session(provider).await {
                Ok(session) => {
                    for job in mine {
                        session.enqueue(&job.id);
                    }
                    started.push(session);
                }
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        if started.is_empty() {
            return Err(failure.unwrap_or(Error::Cancelled));
        }
        self.mark_dirty();
        // Show the rows as started right away instead of on the next 250 ms tick.
        self.flush_jobs();
        match &failure {
            Some(error) => self.status(error.friendly()),
            None => self.status(Msg::plain("Đang xử lý · Retry, Resume, Stop và Xóa có thể thao tác riêng từng torrent.")),
        }
        let app = self.clone();
        tauri::async_runtime::spawn(async move {
            for session in started {
                session.when_idle().await;
            }
            let jobs = app.queue.snapshot();
            let count = |state| jobs.iter().filter(|j| j.state == state).count();
            app.save_now();
            app.status(Msg::new(
                "{0} hoàn tất · {1} lỗi · {2} đã dừng. Bấm Retry hoặc Resume trên từng dòng.",
                &[&count(JobState::Completed), &count(JobState::Failed), &count(JobState::Stopped)],
            ));
        });
        Ok(())
    }

    async fn ensure_session(&self, provider: Provider) -> Result<Arc<Session>> {
        if let Some(session) = self.sessions.lock().await.get(&provider).filter(|s| s.active_count() > 0) {
            return Ok(session.clone());
        }
        // The disk probe and the account check (a network call on the first start) run without holding the session
        // lock, so Stop, Pause and Remove never wait behind them.
        let (root, concurrency, flatten, independent) = self.output_root()?;
        let (account, api) = self.verified(provider).await?;
        if !account.user.can_download() {
            return Err(Error::operation("Cần tài khoản có quyền tải torrent trên {0}.", &[&provider.label()]));
        }
        let mut guard = self.sessions.lock().await;
        // Another Start may have created a session meanwhile; join it instead of running two.
        if let Some(session) = guard.get(&provider).filter(|s| s.active_count() > 0) {
            return Ok(session.clone());
        }
        // Overlapping sessions share file names and one slot policy (per cloud or total). Once every session is
        // idle, the next starts fresh with the current settings.
        let (allocator, slots) = match guard.values().find(|s| s.active_count() > 0) {
            Some(busy) => busy.shared(),
            None => (Arc::new(Allocator::new(&self.queue.snapshot())), Arc::new(DownloadSlots::new(concurrency, independent))),
        };
        let engine = Engine::new(api, self.transfer_http.clone(), Some(account.user.username.clone()), self.queue.clone(), self.timing);
        let session = engine.shared_session(root, flatten, allocator, slots);
        guard.insert(provider, session.clone());
        Ok(session)
    }

    async fn all_sessions(&self) -> Vec<Arc<Session>> {
        self.sessions.lock().await.values().cloned().collect()
    }

    async fn session_for(&self, provider: Provider) -> Option<Arc<Session>> {
        self.sessions.lock().await.get(&provider).cloned()
    }

    pub async fn queue_pause(&self) {
        let sessions = self.all_sessions().await;
        if sessions.is_empty() {
            return;
        }
        self.status(Msg::plain("Đang tạm dừng và lưu tiến trình…"));
        for session in sessions {
            session.pause_all().await;
        }
    }

    pub async fn queue_stop(&self, ids: Option<Vec<String>>) {
        let targets: Vec<Job> = self.queue.snapshot().into_iter().filter(|j| j.can_stop() && ids.as_ref().map(|ids| ids.contains(&j.id)).unwrap_or(true)).collect();
        for job in targets {
            match self.session_for(job.provider).await {
                Some(session) => session.stop(&job.id).await,
                None => {
                    self.queue.mutate(&job.id, |j| {
                        j.state = JobState::Stopped;
                        j.detail = Msg::plain("Đã dừng · bấm Resume để tải tiếp");
                    });
                }
            }
        }
        self.mark_dirty();
        self.status(Msg::plain("Đã dừng tác vụ đã chọn. File tạm được giữ để Resume; torrent trên dịch vụ tiếp tục xử lý."));
    }

    /// Stop writers first, then drop the rows; files on disk and torrents on the service are kept.
    pub async fn queue_remove(&self, ids: Vec<String>) -> Vec<String> {
        let targets: Vec<(String, Provider)> = self.queue.snapshot().into_iter().filter(|j| ids.contains(&j.id) && j.can_remove()).map(|j| (j.id, j.provider)).collect();
        for (id, _) in &targets {
            self.queue.mutate(id, |j| j.is_removing = true);
        }
        for (id, provider) in &targets {
            if let Some(session) = self.session_for(*provider).await {
                session.stop(id).await;
            }
        }
        let targets: Vec<String> = targets.into_iter().map(|(id, _)| id).collect();
        self.queue.remove(&targets);
        self.notifier.pending.lock().unwrap().retain(|id, _| !targets.contains(id));
        self.mark_dirty();
        self.emit("jobs-removed", targets.clone());
        targets
    }

    pub async fn queue_clear(&self, only_completed: bool) -> Vec<String> {
        let ids: Vec<String> = self
            .queue
            .snapshot()
            .into_iter()
            .filter(|j| !only_completed || (j.state == JobState::Completed && !j.is_active))
            .map(|j| j.id)
            .collect();
        self.queue_remove(ids).await
    }

    // ---- remote torrents (explicit service, independent of Settings) -------------------------------------------

    pub async fn torbox_account(&self) -> Result<TorBoxAccount> {
        let (_, api) = self.verified(Provider::TorBox).await?;
        api.torbox_account().await
    }

    pub async fn remote_list(&self, provider: Provider, kind: CloudKind) -> Result<RemoteList> {
        let (account, api) = self.verified(provider).await?;
        let torrents = api.cloud_list(kind).await?;
        Ok(RemoteList { account: account.user.username, torrents })
    }

    /// The verified account must still be the one the list was loaded for, so a token swap can't play another account's file.
    async fn stream_api(&self, provider: Provider, list_account: Option<&str>) -> Result<Arc<dyn Debrid>> {
        let (account, api) = self.verified(provider).await?;
        if let Some(list_account) = list_account.filter(|a| !a.eq_ignore_ascii_case(&account.user.username)) {
            return Err(Error::operation("Token hiện tại thuộc tài khoản {0}, không phải {1}. Tải lại danh sách trước khi phát.", &[&account.user.username, &list_account]));
        }
        Ok(api)
    }

    /// Video files of a cloud torrent, each tied to the link that `stream_url` will resolve; or, while the service is
    /// still fetching it, its progress so the player can wait. With `select`, a torrent the Library left at file
    /// selection gets every file selected, so a later queue of any subset still finds its links. Never pass `select`
    /// for a torrent a queue job owns: the job makes its own selection.
    pub async fn stream_files(&self, provider: Provider, id: String, list_account: Option<String>, select: bool) -> Result<StreamList> {
        let api = self.stream_api(provider, list_account.as_deref()).await?;
        let mut info = api.info(&id).await?;
        if select && info.status == "waiting_files_selection" {
            api.select_all(&id).await?;
            info = api.info(&id).await?;
        }
        if stream::failed_status(&info.status) {
            return Err(Error::operation("Torrent không tải được trên {0}: {1}", &[&provider.label(), &info.status]));
        }
        self.note_stream_cloud(provider, &id, &info);
        let ready = info.status == "downloaded";
        Ok(StreamList { ready, status: info.status.clone(), progress: info.progress, files: if ready { stream::playable_files(&info) } else { Vec::new() } })
    }

    /// Keep the Library's cloud badge in step with what the player just learned.
    fn note_stream_cloud(&self, provider: Provider, id: &str, info: &rdbatch_core::models::TorrentInfo) {
        let cloud = CloudState { status: info.status.clone(), progress: info.progress, speed: info.speed, seeders: info.seeders };
        let update = {
            let mut data = self.data.lock().unwrap();
            let Some(draft) = data.library.iter_mut().find(|d| d.provider == provider && d.torrent_id.as_deref() == Some(id)) else { return };
            if draft.cloud.as_ref() == Some(&cloud) { return }
            draft.cloud = Some(cloud.clone());
            CloudUpdate { provider, source_key: draft.source_key.clone(), cloud: Some(cloud) }
        };
        self.mark_dirty();
        self.emit("cloud", vec![update]);
    }

    /// A fresh direct link for one file. Links expire, so this is resolved on every Play, never stored.
    pub async fn stream_url(&self, provider: Provider, id: String, link_index: usize, list_account: Option<String>) -> Result<StreamSource> {
        let api = self.stream_api(provider, list_account.as_deref()).await?;
        let info = api.info(&id).await?;
        let (name, bytes) = stream::expected_at(&info, link_index).ok_or_else(|| Error::invalid("Không xác định được file cần phát."))?;
        let link = info.links.get(link_index).ok_or_else(|| Error::invalid("Không xác định được file cần phát."))?;
        let resolved = api.unrestrict(link).await?;
        let same_name = name.eq_ignore_ascii_case(&stream::basename(&resolved.filename));
        let same_size = !(bytes > 0 && resolved.filesize > 0 && bytes != resolved.filesize);
        if !same_name || !same_size {
            return Err(Error::invalid("Link phát không khớp với file đã chọn."));
        }
        Ok(StreamSource { url: resolved.download, filename: resolved.filename, bytes: resolved.filesize })
    }

    /// Hands the stream to the OS default media player through a one-line playlist.
    pub fn open_external_player(&self, url: &str, filename: &str) -> Result<()> {
        use tauri_plugin_opener::OpenerExt;
        if !(url.starts_with("https://") || url.starts_with("http://")) || url.contains(['\r', '\n']) {
            return Err(Error::invalid("Liên kết phát không hợp lệ."));
        }
        let path = std::env::temp_dir().join("DebridDock-stream.m3u");
        let title: String = filename.chars().filter(|c| !c.is_control()).collect();
        std::fs::write(&path, format!("#EXTM3U\n#EXTINF:-1,{title}\n{url}\n")).map_err(|e| Error::invalid(e.to_string()))?;
        self.handle.opener().open_path(path.to_string_lossy().to_string(), None::<&str>).map_err(|e| Error::invalid(e.to_string()))
    }

    /// Delete exact IDs on the service. The account must still be the one the list was loaded for, and an ID known to
    /// belong to another account is never deleted. Ambiguous failures are reported, never replayed.
    pub async fn remote_delete(&self, provider: Provider, kind: CloudKind, ids: Vec<String>, list_account: String) -> Result<Vec<DeleteOutcome>> {
        let (account, api) = self.verified(provider).await?;
        if !account.user.username.eq_ignore_ascii_case(&list_account) {
            return Err(Error::operation("Token hiện tại thuộc tài khoản {0}, không phải {1}. Tải lại danh sách trước khi xóa.", &[&account.user.username, &list_account]));
        }
        let session = self.session_for(provider).await;
        let mut outcomes = Vec::new();
        for id in ids {
            let foreign = {
                let data = self.data.lock().unwrap();
                kind == CloudKind::Torrent && data.remote_refs.iter().any(|r| r.provider == provider && r.id == id && r.account.as_deref().map(|a| !a.eq_ignore_ascii_case(&account.user.username)).unwrap_or(false))
            };
            if foreign {
                outcomes.push(DeleteOutcome { id, ok: false, message: Some(Msg::plain("Torrent này thuộc tài khoản khác.")) });
                continue;
            }
            // Only tasks linked to this exact ID are stopped, and their writers are awaited before the DELETE.
            let linked: Vec<String> = self.queue.snapshot().into_iter().filter(|j| kind == CloudKind::Torrent && j.provider == provider && j.torrent_id.as_deref() == Some(id.as_str()) && j.remote_account.as_deref().map(|a| a.eq_ignore_ascii_case(&account.user.username)).unwrap_or(true)).map(|j| j.id).collect();
            for job_id in &linked {
                self.queue.mutate(job_id, |j| j.is_removing = true);
                if let Some(session) = &session {
                    session.stop(job_id).await;
                }
            }
            match api.cloud_delete(kind, &id).await {
                Ok(()) => {
                    self.queue.remove(&linked);
                    if !linked.is_empty() {
                        self.emit("jobs-removed", linked);
                    }
                    let cleared: Vec<Draft> = {
                        let mut data = self.data.lock().unwrap();
                        data.remote_refs.retain(|r| !(kind == CloudKind::Torrent && r.provider == provider && r.id == id && r.account.as_deref().map(|a| a.eq_ignore_ascii_case(&account.user.username)).unwrap_or(true)));
                        data.library.iter_mut().filter(|d| kind == CloudKind::Torrent && d.provider == provider && d.torrent_id.as_deref() == Some(id.as_str()) && d.remote_account.as_deref().map(|a| a.eq_ignore_ascii_case(&account.user.username)).unwrap_or(true)).map(|d| {
                            d.torrent_id = None;
                            d.remote_account = None;
                            d.cloud = None;
                            d.clone()
                        }).collect()
                    };
                    for draft in cleared {
                        self.emit("draft", draft);
                    }
                    outcomes.push(DeleteOutcome { id, ok: true, message: None });
                }
                Err(error) => {
                    for job_id in &linked {
                        self.queue.mutate(job_id, |j| j.is_removing = false);
                    }
                    outcomes.push(DeleteOutcome { id, ok: false, message: Some(error.friendly()) });
                }
            }
        }
        self.mark_dirty();
        Ok(outcomes)
    }
}

/// Wait longer after each failed read of a torrent (1, 2, 4, 8 then 8 minutes).
fn back_off(draft: &mut Draft, error: Msg) {
    draft.inspection_attempts += 1;
    draft.inspection_error = Some(error);
    let wait = 30u64 * 2u64.pow(draft.inspection_attempts.min(4));
    draft.next_inspection_at = Some(Utc::now() + chrono::Duration::seconds(wait as i64));
}

fn uuid_like() -> String {
    format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0))
}

#[allow(dead_code)]
fn _search_is_used_by_the_ui(_: &str) -> bool {
    search::is_video("x.mkv")
}
