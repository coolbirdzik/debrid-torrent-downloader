//! Application state: settings, library, queue, accounts, and the background tasks that keep the UI and disk in sync.
//!
//! Progress never travels per chunk: the queue's change callback only records the latest copy of each job, and one
//! task emits those copies about every 250 ms. Persistence is debounced the same way, except when the engine asks for
//! an immediate flush (a freshly created remote torrent ID).

use chrono::Utc;
use rdbatch_core::engine::{Engine, Queue, Session};
use rdbatch_core::error::{Error, Result};
use rdbatch_core::models::{Draft, Job, JobState, Msg, Persisted, Provider, RemoteRef, RemoteTorrent, Settings, User};
use rdbatch_core::provider::{self, Debrid, Timing};
use rdbatch_core::store::Store;
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

#[derive(Clone)]
struct Account {
    provider: Provider,
    token: String,
    user: User,
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InspectStatus {
    pub running: bool,
    pub done: usize,
    pub total: usize,
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
    pub remember_token: Option<bool>,
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
    account: Mutex<Option<Account>>,
    session: tokio::sync::Mutex<Option<Arc<Session>>>,
    warning: Mutex<Option<Msg>>,
    api_http: reqwest::Client,
    transfer_http: reqwest::Client,
    inspect_running: AtomicBool,
    inspect_cancel: Mutex<CancellationToken>,
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
            account: Mutex::new(None),
            session: tokio::sync::Mutex::new(None),
            warning: Mutex::new(loaded.warning),
            api_http,
            transfer_http,
            inspect_running: AtomicBool::new(false),
            inspect_cancel: Mutex::new(CancellationToken::new()),
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
        let provider = settings.provider;
        let user = self.account.lock().unwrap().as_ref().filter(|a| a.provider == provider).map(|a| a.user.clone());
        Snapshot {
            account: AccountDto { provider, has_token: self.token_for(provider).is_some(), user },
            settings,
            library,
            jobs: self.queue.snapshot(),
            warning: self.warning.lock().unwrap().clone(),
            data_file: self.store.path().display().to_string(),
        }
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Settings {
        let (settings, provider_changed, forget_tokens) = {
            let mut data = self.data.lock().unwrap();
            let s = &mut data.settings;
            let before = (s.provider, s.remember_token);
            if let Some(v) = patch.provider { s.provider = v }
            if let Some(v) = patch.language { s.language = if v == "vi" { "vi".into() } else { "en".into() } }
            if let Some(v) = patch.appearance { s.appearance = if matches!(v.as_str(), "light" | "dark") { v } else { "system".into() } }
            if let Some(v) = patch.browser_view { s.browser_view = if v == "grid" { "grid".into() } else { "list".into() } }
            if let Some(v) = patch.browser_sort { s.browser_sort = if matches!(v.as_str(), "added-desc" | "added-asc") { v } else { "name".into() } }
            if let Some(v) = patch.output_directory { s.output_directory = v.trim().to_string() }
            if let Some(v) = patch.flatten_files { s.flatten_files = v }
            if let Some(v) = patch.concurrency { s.concurrency = v.clamp(1, 5) }
            if let Some(v) = patch.remember_token { s.remember_token = v }
            (s.clone(), before.0 != s.provider, before.1 && !s.remember_token)
        };
        if provider_changed {
            *self.account.lock().unwrap() = None;
        }
        if forget_tokens {
            for provider in [Provider::RealDebrid, Provider::TorBox, Provider::AllDebrid] {
                if let Some(e) = entry(provider) {
                    let _ = e.delete_credential();
                }
            }
        }
        self.mark_dirty();
        settings
    }

    fn token_for(&self, provider: Provider) -> Option<String> {
        if let Some(token) = test_token() {
            return Some(token);
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

    /// Store the token for the current provider. A keychain failure is reported but never loses the token for this
    /// session.
    pub fn save_token(&self, token: &str) -> Option<Msg> {
        let provider = self.provider();
        let token = token.trim().to_string();
        *self.account.lock().unwrap() = None;
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

    /// Verify the stored token and remember the account it belongs to.
    pub async fn connect(&self) -> Result<User> {
        let provider = self.provider();
        let token = self.token_for(provider).ok_or_else(|| Error::Operation(Msg::plain("Dán API token của dịch vụ đã chọn trước khi tải.")))?;
        let user = self.client(provider, &token).user().await?;
        *self.account.lock().unwrap() = Some(Account { provider, token, user: user.clone() });
        // Starting the queue or loading remote torrents verifies the token too; tell the UI which account is connected.
        self.emit("account", user.clone());
        Ok(user)
    }

    async fn verified(&self) -> Result<(Account, Arc<dyn Debrid>)> {
        let provider = self.provider();
        let token = self.token_for(provider).ok_or_else(|| Error::Operation(Msg::plain("Dán API token của dịch vụ đã chọn trước khi tải.")))?;
        let cached = self.account.lock().unwrap().clone().filter(|a| a.provider == provider && a.token == token);
        let account = match cached {
            Some(account) => account,
            None => {
                self.connect().await?;
                self.account.lock().unwrap().clone().expect("connected")
            }
        };
        let client = self.client(provider, &account.token);
        Ok((account, client))
    }

    // ---- library -----------------------------------------------------------------------------------------------

    pub fn library_add(self: &Arc<Self>, text: &str) -> AddResult {
        let parsed = parser::parse(text);
        let provider = self.provider();
        let (mut added, mut duplicates) = (0, 0);
        let mut fresh: Vec<Draft> = Vec::new();
        {
            let mut data = self.data.lock().unwrap();
            data.settings.library_import_paused = false;
            for source in parsed.sources {
                if data.library.iter().any(|d| d.provider == provider && same_key(&d.source_key, &source.key)) {
                    duplicates += 1;
                    continue;
                }
                let draft = Draft { added_at: Some(Utc::now()), provider, source: source.source, source_key: source.key, name: source.name, ..Default::default() };
                data.library.push(draft.clone());
                fresh.push(draft);
                added += 1;
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
        tauri::async_runtime::spawn(async move {
            self.run_inspection().await;
            self.inspect_running.store(false, Ordering::SeqCst);
            self.emit("inspect", InspectStatus { running: false, done: 0, total: 0 });
        });
    }

    fn pending_drafts(&self) -> Vec<(Provider, String)> {
        let data = self.data.lock().unwrap();
        let now = Utc::now();
        data.library
            .iter()
            .filter(|d| d.provider == data.settings.provider && d.files.is_empty() && !d.is_hidden && d.next_inspection_at.map(|t| t <= now).unwrap_or(true))
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
            let (account, api) = match self.verified().await {
                Ok(pair) => pair,
                Err(error) => {
                    for (provider, key) in &pending {
                        self.with_draft(*provider, key, |d| d.inspection_error = Some(error.friendly()));
                    }
                    return;
                }
            };
            let engine = Engine::new(api, self.transfer_http.clone(), Some(account.user.username.clone()), self.queue.clone(), self.timing);
            let total = pending.len();
            for (done, (provider, key)) in pending.into_iter().enumerate() {
                if cancel.is_cancelled() {
                    return;
                }
                self.emit("inspect", InspectStatus { running: true, done, total });
                let existing = self.with_draft(provider, &key, |d| (d.source.clone(), d.torrent_id.clone())).unwrap_or_default();
                let outcome = async {
                    let torrent_id = match existing.1 {
                        Some(id) => id,
                        None => {
                            let id = engine.add_source(&existing.0).await?;
                            let name = self.with_draft(provider, &key, |d| {
                                d.torrent_id = Some(id.clone());
                                d.remote_account = Some(account.user.username.clone());
                                d.name.clone()
                            });
                            self.data.lock().unwrap().remote_refs.push(RemoteRef {
                                provider,
                                id: id.clone(),
                                source_key: key.clone(),
                                name: name.unwrap_or_default(),
                                account: Some(account.user.username.clone()),
                            });
                            // Persist the remote ID before any further request; retries reuse it.
                            self.save_now();
                            id
                        }
                    };
                    engine.inspect(&torrent_id, &cancel).await
                }
                .await;
                match outcome {
                    Ok(info) => {
                        self.with_draft(provider, &key, |d| {
                            if !info.filename.trim().is_empty() {
                                d.name = info.filename.clone();
                            }
                            d.files = info.files.clone();
                            d.inspection_error = None;
                            d.inspection_attempts = 0;
                            d.next_inspection_at = None;
                        });
                        self.notifier.marks_stale.store(true, Ordering::SeqCst);
                    }
                    Err(Error::Cancelled) => return,
                    Err(error) => {
                        self.with_draft(provider, &key, |d| {
                            d.inspection_attempts += 1;
                            d.inspection_error = Some(error.friendly());
                            let wait = 30u64 * 2u64.pow(d.inspection_attempts.min(4));
                            d.next_inspection_at = Some(Utc::now() + chrono::Duration::seconds(wait as i64));
                        });
                    }
                }
            }
            self.save_now();
        }
    }

    // ---- queue -------------------------------------------------------------------------------------------------

    /// Returns the IDs of the new jobs, so the UI can start exactly those.
    pub fn queue_add(&self, items: Vec<QueueItem>) -> Vec<String> {
        let mut created = Vec::new();
        let mut touched = Vec::new();
        {
            let mut data = self.data.lock().unwrap();
            let jobs = self.queue.snapshot();
            for item in items {
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
                job.total_bytes = files.iter().map(|f| f.bytes).sum();
                let names = files.iter().map(|f| f.path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_string()).collect::<Vec<_>>().join(", ");
                job.detail = Msg::new("Đã chọn {0} file · {1}", &[&files.len(), &names]);
                created.push(job.id.clone());
                self.queue.push(job);
                // A later selection needs its own remote torrent; an active selection cannot be changed.
                draft.torrent_id = None;
                draft.remote_account = None;
                touched.push(draft.clone());
            }
        }
        for draft in touched {
            self.emit("draft", draft);
        }
        self.mark_dirty();
        created
    }

    fn output_root(&self) -> Result<(PathBuf, usize, bool)> {
        let (dir, concurrency, flatten) = {
            let data = self.data.lock().unwrap();
            (data.settings.output_directory.trim().to_string(), data.settings.concurrency as usize, data.settings.flatten_files)
        };
        let path = PathBuf::from(&dir);
        if dir.is_empty() || !path.is_absolute() {
            return Err(Error::Operation(Msg::plain("Chọn một đường dẫn lưu đầy đủ.")));
        }
        std::fs::create_dir_all(&path)?;
        let probe = path.join(format!(".rd-write-test-{}", uuid_like()));
        std::fs::write(&probe, b"")?;
        let _ = std::fs::remove_file(&probe);
        Ok((path, concurrency, flatten))
    }

    pub async fn queue_start(self: &Arc<Self>, ids: Option<Vec<String>>) -> Result<()> {
        let provider = self.provider();
        let jobs: Vec<Job> = self
            .queue
            .snapshot()
            .into_iter()
            .filter(|j| j.provider == provider && ids.as_ref().map(|ids| ids.contains(&j.id)).unwrap_or(j.can_resume()) && (j.can_resume() || j.can_retry()))
            .collect();
        if jobs.is_empty() {
            self.status(Msg::plain("Thêm torrent hoặc bấm Retry ở dòng bị lỗi."));
            return Ok(());
        }
        let session = self.ensure_session().await?;
        let mut waits = Vec::new();
        for job in &jobs {
            if let Some(done) = session.enqueue(&job.id) {
                waits.push(done);
            }
        }
        self.mark_dirty();
        // Show the rows as started right away instead of on the next 250 ms tick.
        self.flush_jobs();
        self.status(Msg::plain("Đang xử lý · Retry, Resume, Stop và Xóa có thể thao tác riêng từng torrent."));
        let app = self.clone();
        tauri::async_runtime::spawn(async move {
            session.when_idle().await;
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

    async fn ensure_session(&self) -> Result<Arc<Session>> {
        if let Some(session) = self.session.lock().await.as_ref().filter(|s| s.active_count() > 0) {
            return Ok(session.clone());
        }
        // The disk probe and the account check (a network call on the first start) run without holding the session
        // lock, so Stop, Pause and Remove never wait behind them.
        let (root, concurrency, flatten) = self.output_root()?;
        let (account, api) = self.verified().await?;
        if !account.user.can_download() {
            return Err(Error::operation("Cần tài khoản có quyền tải torrent trên {0}.", &[&account.provider.label()]));
        }
        let mut guard = self.session.lock().await;
        // Another Start may have created a session meanwhile; join it instead of running two.
        if let Some(session) = guard.as_ref().filter(|s| s.active_count() > 0) {
            return Ok(session.clone());
        }
        let engine = Engine::new(api, self.transfer_http.clone(), Some(account.user.username.clone()), self.queue.clone(), self.timing);
        let session = engine.session(root, concurrency, flatten);
        *guard = Some(session.clone());
        Ok(session)
    }

    pub async fn queue_pause(&self) {
        if let Some(session) = self.session.lock().await.clone() {
            self.status(Msg::plain("Đang tạm dừng và lưu tiến trình…"));
            session.pause_all().await;
        }
    }

    pub async fn queue_stop(&self, ids: Option<Vec<String>>) {
        let session = self.session.lock().await.clone();
        let provider = self.provider();
        let targets: Vec<Job> = self.queue.snapshot().into_iter().filter(|j| j.provider == provider && j.can_stop() && ids.as_ref().map(|ids| ids.contains(&j.id)).unwrap_or(true)).collect();
        for job in targets {
            match &session {
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
        let session = self.session.lock().await.clone();
        let targets: Vec<String> = self.queue.snapshot().into_iter().filter(|j| ids.contains(&j.id) && j.can_remove()).map(|j| j.id).collect();
        for id in &targets {
            self.queue.mutate(id, |j| j.is_removing = true);
        }
        if let Some(session) = &session {
            for id in &targets {
                session.stop(id).await;
            }
        }
        self.queue.remove(&targets);
        self.notifier.pending.lock().unwrap().retain(|id, _| !targets.contains(id));
        self.mark_dirty();
        self.emit("jobs-removed", targets.clone());
        targets
    }

    pub async fn queue_clear(&self, only_completed: bool) -> Vec<String> {
        let provider = self.provider();
        let ids: Vec<String> = self
            .queue
            .snapshot()
            .into_iter()
            .filter(|j| j.provider == provider && (!only_completed || (j.state == JobState::Completed && !j.is_active)))
            .map(|j| j.id)
            .collect();
        self.queue_remove(ids).await
    }

    // ---- remote torrents ---------------------------------------------------------------------------------------

    pub async fn remote_list(&self) -> Result<RemoteList> {
        let (account, api) = self.verified().await?;
        let torrents = api.list().await?;
        Ok(RemoteList { account: account.user.username, torrents })
    }

    /// Delete exact IDs on the service. The account must still be the one the list was loaded for, and an ID known to
    /// belong to another account is never deleted. Ambiguous failures are reported, never replayed.
    pub async fn remote_delete(&self, ids: Vec<String>, list_account: String) -> Result<Vec<DeleteOutcome>> {
        let (account, api) = self.verified().await?;
        if !account.user.username.eq_ignore_ascii_case(&list_account) {
            return Err(Error::operation("Token hiện tại thuộc tài khoản {0}, không phải {1}. Tải lại danh sách trước khi xóa.", &[&account.user.username, &list_account]));
        }
        let provider = account.provider;
        let session = self.session.lock().await.clone();
        let mut outcomes = Vec::new();
        for id in ids {
            let foreign = {
                let data = self.data.lock().unwrap();
                data.remote_refs.iter().any(|r| r.provider == provider && r.id == id && r.account.as_deref().map(|a| !a.eq_ignore_ascii_case(&account.user.username)).unwrap_or(false))
            };
            if foreign {
                outcomes.push(DeleteOutcome { id, ok: false, message: Some(Msg::plain("Torrent này thuộc tài khoản khác.")) });
                continue;
            }
            // Only tasks linked to this exact ID are stopped, and their writers are awaited before the DELETE.
            let linked: Vec<String> = self.queue.snapshot().into_iter().filter(|j| j.provider == provider && j.torrent_id.as_deref() == Some(id.as_str())).map(|j| j.id).collect();
            for job_id in &linked {
                self.queue.mutate(job_id, |j| j.is_removing = true);
                if let Some(session) = &session {
                    session.stop(job_id).await;
                }
            }
            match api.delete(&id).await {
                Ok(()) => {
                    self.queue.remove(&linked);
                    if !linked.is_empty() {
                        self.emit("jobs-removed", linked);
                    }
                    let cleared: Vec<Draft> = {
                        let mut data = self.data.lock().unwrap();
                        data.remote_refs.retain(|r| !(r.provider == provider && r.id == id));
                        data.library.iter_mut().filter(|d| d.provider == provider && d.torrent_id.as_deref() == Some(id.as_str())).map(|d| {
                            d.torrent_id = None;
                            d.remote_account = None;
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

fn uuid_like() -> String {
    format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0))
}

#[allow(dead_code)]
fn _search_is_used_by_the_ui(_: &str) -> bool {
    search::is_video("x.mkv")
}
