//! Data shared by the engine, the store and the UI. Field names serialize as camelCase.

use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    #[default]
    RealDebrid,
    TorBox,
    AllDebrid,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Provider::TorBox => "TorBox",
            Provider::AllDebrid => "AllDebrid",
            Provider::RealDebrid => "Real-Debrid",
        }
    }
    pub fn token_page(self) -> &'static str {
        match self {
            Provider::TorBox => "https://torbox.app/settings",
            Provider::AllDebrid => "https://alldebrid.com/apikeys/",
            Provider::RealDebrid => "https://real-debrid.com/apitoken",
        }
    }
}

/// A message the UI renders in the active language. `key` is the Vietnamese source text; unknown keys render as-is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Msg {
    pub key: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl Msg {
    pub fn new(key: &str, args: &[&dyn ToString]) -> Self {
        Msg { key: key.to_string(), args: args.iter().map(|a| a.to_string()).collect() }
    }
    pub fn plain(key: &str) -> Self {
        Msg { key: key.to_string(), args: vec![] }
    }
    /// Replace `{0}`, `{1}`, ... with the arguments, for logs and tests (the UI does this with translations).
    pub fn render(&self) -> String {
        let mut out = self.key.clone();
        for (index, arg) in self.args.iter().enumerate() {
            out = out.replace(&format!("{{{index}}}"), arg);
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    #[default]
    Queued,
    Adding,
    Waiting,
    Downloading,
    Paused,
    Completed,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JobFile {
    pub torrent_file_id: Option<i32>,
    pub link_index: i32,
    pub link: String,
    pub relative_path: String,
    pub expected_bytes: u64,
    pub downloaded_bytes: u64,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub provider: Provider,
    pub source: String,
    pub source_key: String,
    pub torrent_id: Option<String>,
    pub remote_account: Option<String>,
    /// `None` keeps the all-files behavior of queues created before file selection existed.
    pub selected_file_ids: Option<Vec<i32>>,
    pub output_directory: Option<String>,
    /// Folder the user picked when adding this job; used instead of the Settings folder as the download root.
    #[serde(default)]
    pub target_directory: Option<String>,
    pub flatten_files: Option<bool>,
    pub files: Vec<JobFile>,
    pub name: String,
    pub state: JobState,
    pub detail: Msg,
    pub remote_progress: f64,
    pub local_progress: f64,
    pub total_bytes: u64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_stopping: bool,
    #[serde(default)]
    pub is_removing: bool,
}

impl Job {
    pub fn new(provider: Provider, source: &str, source_key: &str, name: &str) -> Self {
        Job {
            id: uuid::Uuid::new_v4().simple().to_string(),
            provider,
            source: source.to_string(),
            source_key: source_key.to_string(),
            torrent_id: None,
            remote_account: None,
            selected_file_ids: None,
            output_directory: None,
            target_directory: None,
            flatten_files: None,
            files: vec![],
            name: name.to_string(),
            state: JobState::Queued,
            detail: Msg::plain("Sẵn sàng"),
            remote_progress: 0.0,
            local_progress: 0.0,
            total_bytes: 0,
            is_active: false,
            is_stopping: false,
            is_removing: false,
        }
    }
    pub fn can_retry(&self) -> bool {
        !self.is_removing && !self.is_active && !self.is_stopping && self.state == JobState::Failed
    }
    pub fn can_resume(&self) -> bool {
        !self.is_removing
            && !self.is_active
            && !self.is_stopping
            && matches!(self.state, JobState::Queued | JobState::Paused | JobState::Stopped)
    }
    pub fn can_stop(&self) -> bool {
        !self.is_removing
            && !self.is_stopping
            && (self.is_active || matches!(self.state, JobState::Queued | JobState::Paused))
    }
    pub fn can_remove(&self) -> bool {
        !self.is_removing && !self.is_stopping
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFile {
    pub id: i32,
    pub path: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub selected: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TorrentInfo {
    pub id: String,
    pub filename: String,
    pub status: String,
    pub progress: f64,
    pub bytes: u64,
    pub files: Vec<TorrentFile>,
    pub links: Vec<String>,
    pub seeders: Option<i64>,
    pub speed: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteTorrent {
    pub id: String,
    pub filename: String,
    pub bytes: u64,
    pub progress: f64,
    pub status: String,
    pub added: Option<DateTime<FixedOffset>>,
    pub speed: Option<u64>,
    pub seeders: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudKind {
    #[default]
    Torrent,
    WebDownload,
    Usenet,
}

/// TorBox's quota values are supplied by user/stats, never inferred from lifetime traffic or a plan price.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TorBoxAccount {
    pub account: String,
    pub email: Option<String>,
    pub plan: String,
    pub expires_at: Option<DateTime<FixedOffset>>,
    pub airlock_used_bytes: Option<u64>,
    pub airlock_limit_bytes: Option<u64>,
    pub airlock_remaining_bytes: Option<u64>,
    pub bandwidth_30d_bytes: Option<u64>,
    pub downloaded_bytes: Option<u64>,
    pub uploaded_bytes: Option<u64>,
    pub warning: Option<Msg>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unrestricted {
    pub filename: String,
    pub filesize: u64,
    pub download: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub kind: String,
    pub premium: i64,
    pub allows_downloads: bool,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub expiration: Option<String>,
    #[serde(default)]
    pub points: Option<u64>,
}

impl User {
    pub fn can_download(&self) -> bool {
        self.allows_downloads || (self.kind == "premium" && self.premium > 0)
    }
}

/// A torrent the user inspected: its files, what was removed from view and which files are already downloaded.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub added_at: Option<DateTime<Utc>>,
    pub provider: Provider,
    pub source: String,
    pub source_key: String,
    pub name: String,
    pub torrent_id: Option<String>,
    pub remote_account: Option<String>,
    pub files: Vec<TorrentFile>,
    pub excluded_file_ids: BTreeSet<i32>,
    pub downloaded_file_ids: BTreeSet<i32>,
    pub partial_file_ids: BTreeSet<i32>,
    pub is_hidden: bool,
    pub inspection_attempts: u32,
    pub next_inspection_at: Option<DateTime<Utc>>,
    pub inspection_error: Option<Msg>,
    /// Latest state of the torrent on the service (its download to the cloud), refreshed in the background.
    #[serde(default)]
    pub cloud: Option<CloudState>,
}

/// How far a service has fetched a torrent into its own storage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CloudState {
    /// The provider's normalized status, such as `downloading`, `downloaded` or `waiting_files_selection`.
    pub status: String,
    pub progress: f64,
    pub speed: Option<u64>,
    pub seeders: Option<i64>,
}

impl CloudState {
    /// Nothing will change on the service by itself, so polling can stop. Real-Debrid waits for a file selection,
    /// which only a queued download makes.
    pub fn is_settled(&self) -> bool {
        matches!(self.status.as_str(), "downloaded" | "waiting_files_selection" | "magnet_error" | "error" | "virus" | "dead" | "missing")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRef {
    pub provider: Provider,
    pub id: String,
    pub source_key: String,
    pub name: String,
    pub account: Option<String>,
}

/// UI language codes the app can store. Keep in sync with `src/lib/languages.ts`.
pub const SUPPORTED_LANGUAGES: [&str; 11] = ["vi", "en", "zh", "tr", "es", "de", "fr", "ko", "ja", "pt", "ru"];

/// Returns `value` when it is a supported UI language, otherwise English.
pub fn normalize_language(value: &str) -> String {
    if SUPPORTED_LANGUAGES.contains(&value) { value.into() } else { "en".into() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub provider: Provider,
    pub language: String,
    pub appearance: String,
    pub browser_view: String,
    pub browser_sort: String,
    pub output_directory: String,
    pub flatten_files: bool,
    pub concurrency: u32,
    /// Give each cloud its own concurrency limit instead of sharing one total.
    pub independent_cloud_downloads: bool,
    pub remember_token: bool,
    pub library_import_paused: bool,
    /// Look for a newer GitHub release when the app starts.
    pub check_updates: bool,
    /// Services the last Library import used; offered first next time.
    pub library_providers: Vec<Provider>,
    /// Reuse `library_providers` without asking when torrents are added to the Library.
    pub remember_library_providers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            provider: Provider::RealDebrid,
            language: "en".into(),
            appearance: "system".into(),
            browser_view: "list".into(),
            browser_sort: "added-desc".into(),
            output_directory: String::new(),
            flatten_files: true,
            concurrency: 3,
            independent_cloud_downloads: true,
            remember_token: true,
            library_import_paused: false,
            check_updates: true,
            library_providers: Vec::new(),
            remember_library_providers: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Persisted {
    pub settings: Settings,
    pub jobs: Vec<Job>,
    pub library: Vec<Draft>,
    pub remote_torrents: Vec<RemoteRef>,
}

pub fn format_bytes(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_actions_follow_stopped_failed_completed_and_removing_states() {
        let mut job = Job::new(Provider::RealDebrid, "s", "k", "n");
        assert!(job.can_resume() && job.can_stop() && !job.can_retry() && job.can_remove());
        job.state = JobState::Failed;
        assert!(job.can_retry() && !job.can_resume() && !job.can_stop());
        job.state = JobState::Completed;
        assert!(!job.can_retry() && !job.can_resume() && !job.can_stop() && job.can_remove());
        job.state = JobState::Downloading;
        job.is_active = true;
        assert!(job.can_stop() && !job.can_resume() && job.can_remove());
        job.is_stopping = true;
        assert!(!job.can_stop() && !job.can_remove());
        job.is_stopping = false;
        job.is_removing = true;
        assert!(!job.can_stop() && !job.can_remove());
    }

    #[test]
    fn byte_sizes_are_formatted_like_the_old_app() {
        assert_eq!(format_bytes(0.0), "0 B");
        assert_eq!(format_bytes(1536.0), "1.5 KB");
        assert_eq!(format_bytes(12.5 * 1024.0 * 1024.0), "12.5 MB");
    }
}
