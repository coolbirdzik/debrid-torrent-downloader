//! Persistence: one JSON file, written atomically (temp file then rename). An unreadable file is kept as a
//! `.corrupt-*` copy and never silently overwritten.

use crate::error::{Error, Result};
use crate::models::{JobState, Msg, Persisted};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Store {
    path: PathBuf,
    save_blocked: AtomicBool,
}

pub struct Loaded {
    pub data: Persisted,
    pub warning: Option<Msg>,
}

impl Store {
    pub fn new(directory: &Path) -> Self {
        Store { path: directory.join("data.json"), save_blocked: AtomicBool::new(false) }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Loaded {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Loaded { data: Persisted::default(), warning: None },
            Err(_) => return self.keep_original(),
        };
        match serde_json::from_str::<Persisted>(&text) {
            Ok(mut data) => {
                normalize(&mut data);
                Loaded { data, warning: None }
            }
            Err(_) => self.keep_original(),
        }
    }

    fn keep_original(&self) -> Loaded {
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
        let backup = PathBuf::from(format!("{}.corrupt-{stamp}", self.path.display()));
        let warning = match std::fs::copy(&self.path, &backup) {
            Ok(_) => Msg::new("Không đọc được hàng đợi cũ. Đã giữ bản gốc: {0}", &[&backup.display()]),
            Err(_) => {
                self.save_blocked.store(true, Ordering::SeqCst);
                Msg::plain("Không đọc hoặc sao lưu được hàng đợi cũ. Tạm ngừng ghi settings.json để giữ bản gốc.")
            }
        };
        Loaded { data: Persisted::default(), warning: Some(warning) }
    }

    pub fn save(&self, data: &Persisted) -> Result<()> {
        if self.save_blocked.load(Ordering::SeqCst) {
            return Err(Error::Operation(Msg::plain("Không đọc hoặc sao lưu được hàng đợi cũ. Tạm ngừng ghi settings.json để giữ bản gốc.")));
        }
        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(data)?)?;
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}

fn normalize(data: &mut Persisted) {
    let settings = &mut data.settings;
    settings.concurrency = settings.concurrency.clamp(1, 5);
    settings.language = crate::models::normalize_language(&settings.language);
    if !matches!(settings.appearance.as_str(), "light" | "dark") {
        settings.appearance = "system".into();
    }
    settings.browser_view = if settings.browser_view == "grid" { "grid".into() } else { "list".into() };
    if !matches!(settings.browser_sort.as_str(), "name" | "added-desc" | "added-asc") {
        settings.browser_sort = "added-desc".into();
    }
    for job in &mut data.jobs {
        job.is_active = false;
        job.is_stopping = false;
        job.is_removing = false;
        // A job that was mid-flight when the app closed resumes from the Start button.
        if !matches!(job.state, JobState::Completed | JobState::Failed | JobState::Queued | JobState::Stopped) {
            job.state = JobState::Paused;
            job.detail = Msg::plain("Bấm Bắt đầu để tiếp tục");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Job, Provider};

    #[test]
    fn saved_queue_restores_paused_state_and_preserves_remote_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut data = Persisted::default();
        let mut running = Job::new(Provider::RealDebrid, "magnet:?xt=urn:btih:abc", "btih:ABC", "Running");
        running.state = JobState::Downloading;
        running.is_active = true;
        running.torrent_id = Some("RD123".into());
        let mut done = Job::new(Provider::TorBox, "s", "k", "Done");
        done.state = JobState::Completed;
        data.jobs = vec![running, done];
        store.save(&data).unwrap();
        let loaded = store.load();
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.data.jobs[0].state, JobState::Paused);
        assert!(!loaded.data.jobs[0].is_active);
        assert_eq!(loaded.data.jobs[0].torrent_id.as_deref(), Some("RD123"));
        assert_eq!(loaded.data.jobs[1].state, JobState::Completed);
        assert_eq!(loaded.data.jobs[1].provider, Provider::TorBox);
    }

    #[test]
    fn a_corrupt_file_is_kept_and_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        std::fs::write(store.path(), b"{ not json").unwrap();
        let loaded = store.load();
        assert!(loaded.warning.is_some());
        assert!(loaded.data.jobs.is_empty());
        let backups = std::fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().contains(".corrupt-")).count();
        assert_eq!(backups, 1);
        assert_eq!(std::fs::read(store.path()).unwrap(), b"{ not json");
    }

    #[test]
    fn defaults_apply_to_a_minimal_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        std::fs::write(store.path(), br#"{"settings":{"concurrency":99,"language":"xx"}}"#).unwrap();
        let loaded = store.load();
        assert_eq!(loaded.data.settings.concurrency, 5);
        assert_eq!(loaded.data.settings.language, "en");
        assert!(loaded.data.settings.flatten_files, "flat layout stays the default");
        assert!(loaded.data.settings.independent_cloud_downloads, "existing settings opt into independent cloud limits");
    }

    #[test]
    fn cloud_concurrency_mode_survives_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        for independent in [false, true] {
            let mut data = Persisted::default();
            data.settings.independent_cloud_downloads = independent;
            data.settings.concurrency = 5;
            store.save(&data).unwrap();
            let loaded = store.load();
            assert!(loaded.warning.is_none());
            assert_eq!(loaded.data.settings.independent_cloud_downloads, independent);
            assert_eq!(loaded.data.settings.concurrency, 5);
        }
    }

    #[test]
    fn supported_languages_survive_a_reload() {
        for code in crate::models::SUPPORTED_LANGUAGES {
            assert_eq!(crate::models::normalize_language(code), code);
        }
        assert_eq!(crate::models::normalize_language("xx"), "en");
    }
}
