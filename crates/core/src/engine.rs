//! The queue engine: add a torrent to a provider, wait for it, resolve links and download every selected file.
//!
//! Jobs live in a [`Queue`]; every mutation goes through it, so listeners see one consistent stream of updates.
//! A [`Session`] runs jobs with bounded concurrency and supports pause, stop and retry of individual jobs.

use crate::downloader::{self, TransferProgress};
use crate::error::{Error, Result};
use crate::models::{format_bytes, Job, JobFile, JobState, Msg, TorrentInfo};
use crate::paths;
use crate::provider::{Debrid, Timing};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::{watch, Semaphore};
use tokio_util::sync::CancellationToken;

const MAX_TORRENT_BYTES: usize = 20 * 1024 * 1024;
/// A job that fails on a connection problem runs again by itself this many times before it is marked failed.
const AUTO_RETRIES: u32 = 5;
const MAX_RETRY_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// `(job, durable)`: `durable` is false for byte-progress updates, which the UI wants but the disk does not. The job
/// is the sink's own copy, so keeping it costs no second clone.
type ChangeSink = Box<dyn Fn(Job, bool) + Send + Sync>;
type PersistSink = Box<dyn Fn() + Send + Sync>;

/// The job list plus the notifications that keep the UI and the disk in step with it.
pub struct Queue {
    jobs: Mutex<Vec<Job>>,
    on_change: ChangeSink,
    on_persist: PersistSink,
}

impl Queue {
    /// `on_change` fires after every mutation (the app coalesces it); `on_persist` is called when the state must
    /// reach disk immediately, such as right after a remote torrent ID is obtained.
    pub fn new(jobs: Vec<Job>, on_change: impl Fn(Job, bool) + Send + Sync + 'static, on_persist: impl Fn() + Send + Sync + 'static) -> Arc<Queue> {
        Arc::new(Queue { jobs: Mutex::new(jobs), on_change: Box::new(on_change), on_persist: Box::new(on_persist) })
    }
    pub fn snapshot(&self) -> Vec<Job> {
        self.jobs.lock().unwrap().clone()
    }
    pub fn get(&self, id: &str) -> Option<Job> {
        self.jobs.lock().unwrap().iter().find(|job| job.id == id).cloned()
    }
    /// Look at one job without cloning it (jobs can hold thousands of files).
    pub fn read<R>(&self, id: &str, look: impl FnOnce(&Job) -> R) -> Option<R> {
        self.jobs.lock().unwrap().iter().find(|job| job.id == id).map(look)
    }
    pub fn push(&self, job: Job) {
        self.jobs.lock().unwrap().push(job.clone());
        (self.on_change)(job, true);
    }
    pub fn remove(&self, ids: &[String]) {
        self.jobs.lock().unwrap().retain(|job| !ids.contains(&job.id));
    }
    pub fn mutate<R>(&self, id: &str, change: impl FnOnce(&mut Job) -> R) -> Option<R> {
        self.apply(id, true, change)
    }
    /// Byte progress: the UI is told, but nothing is marked for saving (state changes and file completion still are).
    pub fn mutate_progress<R>(&self, id: &str, change: impl FnOnce(&mut Job) -> R) -> Option<R> {
        self.apply(id, false, change)
    }
    fn apply<R>(&self, id: &str, durable: bool, change: impl FnOnce(&mut Job) -> R) -> Option<R> {
        let (result, snapshot) = {
            let mut jobs = self.jobs.lock().unwrap();
            let job = jobs.iter_mut().find(|job| job.id == id)?;
            let result = change(job);
            (result, job.clone())
        };
        (self.on_change)(snapshot, durable);
        Some(result)
    }
    pub fn persist_now(&self) {
        (self.on_persist)();
    }
    pub fn len(&self) -> usize {
        self.jobs.lock().unwrap().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Keeps final names and the downloader's temporary files distinct across a whole batch.
pub struct Allocator {
    reserved: Mutex<HashSet<String>>,
}

impl Allocator {
    pub fn new(jobs: &[Job]) -> Self {
        let allocator = Allocator { reserved: Mutex::new(HashSet::new()) };
        for job in jobs {
            if let Some(root) = &job.output_directory {
                for file in &job.files {
                    if let Ok(path) = paths::under_root(Path::new(root), &file.relative_path) {
                        allocator.reserve(&path);
                    }
                }
            }
        }
        allocator
    }
    fn variants(path: &Path) -> [PathBuf; 3] {
        let text = path.as_os_str().to_os_string();
        let mut part = text.clone();
        part.push(".part");
        let mut meta = part.clone();
        meta.push(".json");
        [path.to_path_buf(), PathBuf::from(part), PathBuf::from(meta)]
    }
    fn key(path: &Path) -> String {
        path.to_string_lossy().to_lowercase()
    }
    fn reserve(&self, path: &Path) {
        let mut reserved = self.reserved.lock().unwrap();
        for variant in Self::variants(path) {
            reserved.insert(Self::key(&variant));
        }
    }
    pub fn allocate(&self, root: &Path, relative: &str) -> Result<String> {
        let mut candidate = relative.to_string();
        let mut suffix = 2;
        loop {
            let full = paths::under_root(root, &candidate)?;
            // Check and reserve under one lock so concurrent jobs cannot pick the same name.
            let mut reserved = self.reserved.lock().unwrap();
            let occupied = Self::variants(&full).iter().any(|p| reserved.contains(&Self::key(p)) || p.exists());
            if !occupied {
                for variant in Self::variants(&full) {
                    reserved.insert(Self::key(&variant));
                }
                return Ok(candidate);
            }
            drop(reserved);
            let (directory, name) = match relative.rsplit_once('/') {
                Some((d, n)) => (format!("{d}/"), n),
                None => (String::new(), relative),
            };
            let (stem, extension) = match name.rfind('.') {
                Some(i) if i > 0 => (&name[..i], &name[i..]),
                _ => (name, ""),
            };
            candidate = format!("{directory}{stem} ({suffix}){extension}");
            suffix += 1;
        }
    }
}

async fn guard<T>(ct: &CancellationToken, work: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::select! {
        biased;
        _ = ct.cancelled() => Err(Error::Cancelled),
        result = work => result,
    }
}

async fn pause(ct: &CancellationToken, wait: std::time::Duration) -> Result<()> {
    guard(ct, async {
        tokio::time::sleep(wait).await;
        Ok(())
    })
    .await
}

fn basename(path: &str) -> String {
    path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_string()
}

fn remote_label(status: &str) -> String {
    match status {
        "magnet_conversion" => "Đang lấy metadata",
        "queued" => "Đang xếp hàng",
        "downloading" => "Dịch vụ đang tải",
        "compressing" => "Đang đóng gói",
        "uploading" => "Đang chuẩn bị link",
        "waiting_files_selection" => "Đang chọn file",
        other => other,
    }
    .to_string()
}

fn failed_status(status: &str) -> bool {
    matches!(status, "magnet_error" | "error" | "virus" | "dead")
}

pub struct Engine {
    api: Arc<dyn Debrid>,
    http: reqwest::Client,
    account: Option<String>,
    queue: Arc<Queue>,
    timing: Timing,
}

impl Engine {
    pub fn new(api: Arc<dyn Debrid>, http: reqwest::Client, account: Option<String>, queue: Arc<Queue>, timing: Timing) -> Arc<Engine> {
        Arc::new(Engine { api, http, account, queue, timing })
    }
    pub fn queue(&self) -> &Arc<Queue> {
        &self.queue
    }

    pub async fn add_source(&self, source: &str) -> Result<String> {
        if source.len() >= 7 && source[..7].eq_ignore_ascii_case("magnet:") {
            return self.api.add_magnet(source).await;
        }
        let bytes = self.read_torrent(source).await?;
        self.api.add_torrent(bytes).await
    }

    /// Inspection never selects files or starts local transfers.
    pub async fn inspect(&self, torrent_id: &str, ct: &CancellationToken) -> Result<TorrentInfo> {
        let work = async {
            loop {
                let info = self.api.info(torrent_id).await?;
                if failed_status(&info.status) {
                    return Err(Error::operation("Torrent không tải được trên {0}: {1}", &[&self.api.provider().label(), &info.status]));
                }
                if !info.files.is_empty() {
                    return Ok(info);
                }
                tokio::time::sleep(self.timing.inspect_poll).await;
            }
        };
        guard(ct, async { tokio::time::timeout(std::time::Duration::from_secs(180), work).await.map_err(|_| Error::Timeout)? }).await
    }

    pub fn session(self: &Arc<Self>, root: PathBuf, concurrency: usize, flatten: bool) -> Arc<Session> {
        let jobs = self.queue.snapshot();
        Arc::new(Session {
            engine: self.clone(),
            root,
            flatten,
            allocator: Allocator::new(&jobs),
            slots: Arc::new(Semaphore::new(concurrency.clamp(1, 5))),
            active: Mutex::new(HashMap::new()),
            idle: watch::channel(true).0,
        })
    }

    async fn read_torrent(&self, source: &str) -> Result<Vec<u8>> {
        let too_big = || Error::Operation(Msg::plain("File .torrent lớn hơn 20 MB."));
        let is_url = url::Url::parse(source).map(|u| matches!(u.scheme(), "http" | "https")).unwrap_or(false);
        if !is_url {
            let length = tokio::fs::metadata(source).await?.len() as usize;
            if length > MAX_TORRENT_BYTES {
                return Err(too_big());
            }
            return Ok(tokio::fs::read(source).await?);
        }
        let response = tokio::time::timeout(std::time::Duration::from_secs(90), self.http.get(source).send()).await.map_err(|_| Error::Timeout)??;
        let response = response.error_for_status()?;
        if response.content_length().unwrap_or(0) as usize > MAX_TORRENT_BYTES {
            return Err(too_big());
        }
        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if buffer.len() + chunk.len() > MAX_TORRENT_BYTES {
                return Err(too_big());
            }
            buffer.extend_from_slice(&chunk);
        }
        Ok(buffer)
    }

    fn set(&self, id: &str, state: JobState, detail: Msg) {
        self.queue.mutate(id, |job| {
            job.state = state;
            job.detail = detail;
        });
    }

    async fn process(&self, id: &str, root: &Path, flatten: bool, allocator: &Allocator, ct: &CancellationToken) -> Result<()> {
        let job = self.queue.get(id).ok_or(Error::Cancelled)?;
        let label = self.api.provider().label();
        if job.provider != self.api.provider() {
            return Err(Error::operation("Chọn dịch vụ {0} để tiếp tục torrent này.", &[&job.provider.label()]));
        }
        if let (Some(_), Some(account), Some(me)) = (&job.torrent_id, &job.remote_account, &self.account) {
            if !account.eq_ignore_ascii_case(me) {
                return Err(Error::operation("Torrent thuộc tài khoản {0}; token hiện tại là {1}.", &[account, me]));
            }
        }
        if job.selected_file_ids.as_ref().is_some_and(|ids| ids.is_empty()) {
            return Err(Error::Operation(Msg::plain("Chọn ít nhất một file để tải.")));
        }
        self.set(
            id,
            JobState::Adding,
            if job.torrent_id.is_none() { Msg::new("Đang thêm vào {0}…", &[&label]) } else { Msg::plain("Đang tiếp tục torrent đã thêm…") },
        );
        let torrent_id = match job.torrent_id.clone() {
            Some(existing) => existing,
            None => {
                let created = guard(ct, self.add_source(&job.source)).await?;
                self.queue.mutate(id, |j| {
                    j.torrent_id = Some(created.clone());
                    j.remote_account = self.account.clone();
                });
                // Persist the ID before any later network call: a crash must not orphan the remote torrent.
                self.queue.persist_now();
                created
            }
        };

        let selected_ids = job.selected_file_ids.clone();
        let mut selected = false;
        let info = loop {
            if ct.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let info = guard(ct, self.api.info(&torrent_id)).await?;
            self.queue.mutate(id, |j| {
                if !info.filename.trim().is_empty() {
                    j.name = info.filename.clone();
                }
                j.remote_progress = info.progress;
                j.total_bytes = match &selected_ids {
                    Some(ids) if !info.files.is_empty() => info.files.iter().filter(|f| ids.contains(&f.id)).map(|f| f.bytes).sum(),
                    _ => info.bytes,
                };
            });
            if failed_status(&info.status) {
                return Err(Error::operation("Torrent không tải được trên {0}: {1}", &[&label, &info.status]));
            }
            let percent = format!("{:.0}", info.progress);
            if info.status == "waiting_files_selection" && !selected {
                match &selected_ids {
                    None => guard(ct, self.api.select_all(&torrent_id)).await?,
                    Some(ids) => {
                        let known: HashSet<i32> = info.files.iter().map(|f| f.id).collect();
                        if ids.iter().any(|selected| !known.contains(selected)) {
                            return Err(Error::Operation(Msg::plain("Danh sách file torrent đã thay đổi. Hãy đọc lại torrent và chọn lại file.")));
                        }
                        guard(ct, self.api.select_files(&torrent_id, ids)).await?;
                    }
                }
                selected = true;
                self.set(id, JobState::Waiting, Msg::plain("Đã chọn file · chờ dịch vụ…"));
            } else if info.status == "downloaded" {
                if selected_ids.is_none() && info.files.iter().any(|f| f.selected != 1) {
                    return Err(Error::Operation(Msg::plain(
                        "Torrent trên tài khoản chỉ chọn một phần file. Hãy thêm lại torrent trên dịch vụ với toàn bộ file được chọn.",
                    )));
                }
                break info;
            } else if info.status == "downloading" && info.seeders == Some(0) && info.speed.unwrap_or(0) == 0 {
                // Waiting for sources is a remote state: nothing is transferred locally yet.
                self.set(id, JobState::Waiting, Msg::new("{0} đang chờ nguồn tải · 0 seed · 0 B/s · {1}%", &[&label, &percent]));
            } else if info.status == "downloading" && (info.seeders.is_some() || info.speed.is_some()) {
                let seeders = info.seeders.map(|s| s.to_string()).unwrap_or_else(|| "?".into());
                let speed = format_bytes(info.speed.unwrap_or(0) as f64);
                self.set(id, JobState::Waiting, Msg::new("{0} · {1}% · {2} seed · {3}/s", &[&label, &percent, &seeders, &speed]));
            } else {
                self.set(id, JobState::Waiting, Msg::new(&format!("{} · {{0}}%", remote_label(&info.status)), &[&percent]));
            }
            // Many torrents may be waiting at once; the client also applies a global rate limit.
            pause(ct, self.timing.poll).await?;
        };

        if info.links.is_empty() {
            return Err(Error::Operation(Msg::plain("Dịch vụ chưa cung cấp link tải cho torrent này.")));
        }
        let source_files: Vec<_> = info.files.iter().filter(|f| f.selected == 1).cloned().collect();
        let requested: Option<HashSet<i32>> = selected_ids.as_ref().map(|ids| ids.iter().copied().collect());
        if let Some(requested) = &requested {
            let available: HashSet<i32> = source_files.iter().map(|f| f.id).collect();
            if requested.iter().any(|r| !available.contains(r)) {
                return Err(Error::Operation(Msg::plain("Các file đã chọn chưa có link tải. Hãy đọc lại torrent và chọn lại file.")));
            }
            // Never download an archive that could include files outside the user's selection.
            if source_files.len() != info.links.len() {
                return Err(Error::Operation(Msg::plain("Dịch vụ trả về link gộp, không thể tải riêng các file đã chọn.")));
            }
            let total: u64 = source_files.iter().filter(|f| requested.contains(&f.id)).map(|f| f.bytes).sum();
            self.queue.mutate(id, |j| j.total_bytes = total);
        }
        let download_count = requested.as_ref().map(|r| r.len()).unwrap_or(info.links.len());
        let absolute_root = std::path::absolute(root)?;
        let output = self
            .queue
            .mutate(id, |j| {
                j.remote_progress = 100.0;
                if j.files.is_empty() {
                    j.flatten_files = Some(flatten);
                    j.output_directory = Some(
                        if flatten {
                            absolute_root.to_string_lossy().to_string()
                        } else {
                            absolute_root.join(format!("{}_{}", paths::segment(&j.name), &j.id[..8.min(j.id.len())])).to_string_lossy().to_string()
                        },
                    );
                }
                // Queues from the first release already have paths in a torrent folder; keep those resumable.
                j.flatten_files.get_or_insert(false);
                j.output_directory.clone()
            })
            .flatten()
            .ok_or_else(|| Error::Operation(Msg::plain("Hàng đợi có file đã lên lịch nhưng thiếu thư mục lưu.")))?;
        let output_dir = PathBuf::from(&output);
        tokio::fs::create_dir_all(&output_dir).await?;
        self.set(id, JobState::Downloading, Msg::plain("Đang chuẩn bị tải file đã chọn…"));

        // Match every link to its queue row in one pass. Doing this per link (a full job clone each time) was quadratic
        // and made Resume of a torrent with thousands of files hang before the first transfer.
        let wanted: Vec<usize> = (0..info.links.len()).filter(|&index| requested.as_ref().is_none_or(|r| r.contains(&source_files[index].id))).collect();
        let (flatten_files, plan) = self
            .queue
            .mutate(id, |j| {
                let by_file_id: HashMap<i32, usize> = j.files.iter().enumerate().filter_map(|(i, f)| f.torrent_file_id.map(|fid| (fid, i))).collect();
                let by_index: HashMap<i32, usize> = j.files.iter().enumerate().rev().map(|(i, f)| (f.link_index, i)).collect();
                let by_link: HashMap<&str, usize> = j.files.iter().enumerate().rev().map(|(i, f)| (f.link.as_str(), i)).collect();
                let aligned = requested.is_none() && j.files.len() == info.links.len();
                // A row belongs to one link only; an unmatched link gets a new row, as when rows were matched one by one.
                let mut claimed = HashSet::new();
                let plan: Vec<(usize, Option<usize>)> = wanted
                    .iter()
                    .map(|&index| {
                        let found = match &requested {
                            Some(_) => by_file_id.get(&source_files[index].id).copied(),
                            None => by_index.get(&(index as i32)).or_else(|| by_link.get(info.links[index].as_str())).copied(),
                        };
                        // Restricted links may change after resume: file IDs identify selections, indices old all-file queues.
                        (index, found.or(if aligned { Some(index) } else { None }).filter(|i| claimed.insert(*i)))
                    })
                    .collect();
                for &(index, found) in &plan {
                    if let Some(i) = found {
                        j.files[i].link_index = index as i32;
                        j.files[i].link = info.links[index].clone();
                    }
                }
                let finished: Vec<(usize, String, u64)> =
                    plan.iter().filter_map(|&(_, found)| found).filter(|&i| j.files[i].completed).map(|i| (i, j.files[i].relative_path.clone(), j.files[i].expected_bytes)).collect();
                (j.flatten_files, (plan, finished))
            })
            .ok_or(Error::Cancelled)?;
        let (plan, finished) = plan;
        // Files marked done are trusted only if they are still on disk with the expected size.
        let check_dir = output_dir.clone();
        let missing: HashSet<usize> = tokio::task::spawn_blocking(move || {
            finished
                .into_iter()
                .filter(|(_, relative, expected)| {
                    let Ok(existing) = paths::under_root(&check_dir, relative) else { return true };
                    !std::fs::metadata(existing).map(|m| *expected == 0 || m.len() == *expected).unwrap_or(false)
                })
                .map(|(i, _, _)| i)
                .collect()
        })
        .await
        .map_err(|e| Error::invalid(e.to_string()))?;
        if !missing.is_empty() {
            self.queue.mutate(id, |j| {
                for &i in &missing {
                    j.files[i].completed = false;
                }
            });
        }
        let completed_now: HashSet<usize> = self.queue.read(id, |j| j.files.iter().enumerate().filter(|(_, f)| f.completed).map(|(i, _)| i).collect()).unwrap_or_default();

        let mut completed_index = 0usize;
        for (index, position) in plan {
            if ct.is_cancelled() {
                return Err(Error::Cancelled);
            }
            completed_index += 1;
            if position.is_some_and(|i| completed_now.contains(&i)) {
                continue;
            }
            let link = info.links[index].clone();
            let mut attempt = 0u32;
            // Set by the first attempt that creates the file's row; later attempts reuse that row (and its partial).
            let mut position = position;
            loop {
                let outcome = self
                    .download_one(id, &output_dir, flatten_files, allocator, &source_files, &info, requested.as_ref(), index, &mut position, &link, completed_index, download_count, ct)
                    .await;
                match outcome {
                    Ok(()) => break,
                    Err(error) if !ct.is_cancelled() && attempt < 2 && error.is_transient() => {
                        self.queue.mutate(id, |j| j.detail = Msg::new("Kết nối gián đoạn · thử lại {0}/2…", &[&(attempt + 1)]));
                        pause(ct, self.timing.transfer_retry * (attempt + 1)).await?;
                        attempt += 1;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        self.queue.mutate(id, |j| {
            j.local_progress = 100.0;
            j.state = JobState::Completed;
            j.detail = Msg::new("Đã lưu {0} file · {1}", &[&download_count, &output]);
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_one(
        &self,
        id: &str,
        output_dir: &Path,
        flatten_files: Option<bool>,
        allocator: &Allocator,
        source_files: &[crate::models::TorrentFile],
        info: &TorrentInfo,
        requested: Option<&HashSet<i32>>,
        index: usize,
        slot: &mut Option<usize>,
        link: &str,
        completed_index: usize,
        download_count: usize,
        ct: &CancellationToken,
    ) -> Result<()> {
        let position = *slot;
        let resolved = guard(ct, self.api.unrestrict(link)).await?;
        if requested.is_some() {
            let expected = &source_files[index];
            let same_name = basename(&expected.path).eq_ignore_ascii_case(&basename(&resolved.filename));
            let same_size = !(expected.bytes > 0 && resolved.filesize > 0 && expected.bytes != resolved.filesize);
            if !same_name || !same_size {
                return Err(Error::Operation(Msg::plain("Link tải không khớp với file đã chọn. Không tải để tránh lưu nhầm file.")));
            }
        }
        let position = match position {
            Some(position) => {
                let expected = self.queue.read(id, |j| j.files.get(position).map(|f| f.expected_bytes)).flatten().unwrap_or(0);
                if expected > 0 && resolved.filesize > 0 && expected != resolved.filesize {
                    return Err(Error::Operation(Msg::plain("Kích thước file đã thay đổi; không ghép với dữ liệu cũ.")));
                }
                if expected == 0 {
                    self.queue.mutate(id, |j| j.files[position].expected_bytes = resolved.filesize);
                }
                position
            }
            None => {
                // RD can return split archives instead of one link per file. Keep torrent directories only when every
                // selected file maps to one link.
                let source_path = &source_files[index].path;
                let relative = if flatten_files != Some(true) && source_files.len() == info.links.len() && basename(source_path).eq_ignore_ascii_case(&resolved.filename) {
                    paths::relative(source_path)?
                } else {
                    paths::segment(&basename(&resolved.filename))
                };
                let relative = allocator.allocate(output_dir, &relative)?;
                let file = JobFile {
                    torrent_file_id: requested.map(|_| source_files[index].id),
                    link_index: index as i32,
                    link: link.to_string(),
                    relative_path: relative,
                    expected_bytes: resolved.filesize,
                    ..Default::default()
                };
                let created = self
                    .queue
                    .mutate(id, |j| {
                        j.files.push(file);
                        j.files.len() - 1
                    })
                    .ok_or(Error::Cancelled)?;
                *slot = Some(created);
                created
            }
        };
        let (relative, expected_bytes, completed_bytes, expected_total) = self
            .queue
            .read(id, |job| {
                let file = &job.files[position];
                let completed: u64 = job.files.iter().filter(|f| f.completed).map(|f| f.expected_bytes).sum();
                let sum: u64 = job.files.iter().map(|f| f.expected_bytes).sum();
                (file.relative_path.clone(), file.expected_bytes, completed, job.total_bytes.max(sum))
            })
            .ok_or(Error::Cancelled)?;
        let destination = paths::under_root(output_dir, &relative)?;
        paths::ensure_no_reparse_points(output_dir, &destination)?;
        let name = basename(&relative);
        let queue = self.queue.clone();
        let job_id = id.to_string();
        let report = move |p: TransferProgress| {
            queue.mutate_progress(&job_id, |job| {
                if let Some(file) = job.files.get_mut(position) {
                    file.downloaded_bytes = p.bytes;
                }
                job.local_progress = if expected_total > 0 {
                    ((completed_bytes + p.bytes) as f64 * 100.0 / expected_total as f64).min(100.0)
                } else {
                    (completed_index - 1) as f64 * 100.0 / download_count as f64
                };
                job.detail = Msg::plain(&format!(
                    "{completed_index}/{download_count} · {name} · {} / {} · {}/s",
                    format_bytes(p.bytes as f64),
                    format_bytes(p.total as f64),
                    format_bytes(p.bytes_per_second)
                ));
            });
        };
        downloader::download(&self.http, &resolved.download, &destination, expected_bytes, &report, ct).await?;
        self.queue.mutate(id, |j| j.files[position].completed = true);
        Ok(())
    }
}

struct Execution {
    ct: CancellationToken,
    cancel_state: Mutex<JobState>,
    done: watch::Sender<bool>,
}

/// Runs queued jobs with bounded concurrency. Pause and Stop keep partial files so Resume continues exactly.
pub struct Session {
    engine: Arc<Engine>,
    root: PathBuf,
    flatten: bool,
    allocator: Allocator,
    slots: Arc<Semaphore>,
    active: Mutex<HashMap<String, Arc<Execution>>>,
    idle: watch::Sender<bool>,
}

fn cancellation_detail(state: JobState) -> Msg {
    Msg::plain(if state == JobState::Stopped {
        "Đã dừng · giữ file tạm · bấm Resume để tải tiếp"
    } else {
        "Đã tạm dừng · giữ file tạm · bấm Resume để tải tiếp"
    })
}

pub async fn wait_done(mut done: watch::Receiver<bool>) {
    let _ = done.wait_for(|finished| *finished).await;
}

impl Session {
    pub fn active_count(&self) -> usize {
        self.active.lock().unwrap().len()
    }

    pub async fn when_idle(&self) {
        let mut idle = self.idle.subscribe();
        let _ = idle.wait_for(|idle| *idle).await;
    }

    /// Start (or join) a job. Returns a receiver that flips to `true` when the job's run has fully ended.
    pub fn enqueue(self: &Arc<Self>, id: &str) -> Option<watch::Receiver<bool>> {
        let execution;
        {
            let mut active = self.active.lock().unwrap();
            if let Some(current) = active.get(id) {
                return Some(current.done.subscribe());
            }
            let job = self.engine.queue.get(id)?;
            if job.state == JobState::Completed || job.is_removing {
                return None;
            }
            if active.is_empty() {
                self.idle.send_replace(false);
            }
            execution = Arc::new(Execution { ct: CancellationToken::new(), cancel_state: Mutex::new(JobState::Paused), done: watch::channel(false).0 });
            active.insert(id.to_string(), execution.clone());
        }
        self.engine.queue.mutate(id, |job| {
            job.is_active = true;
            job.is_stopping = false;
            job.state = JobState::Queued;
            job.detail = Msg::plain("Chờ lượt xử lý…");
        });
        let done = execution.done.subscribe();
        let session = self.clone();
        let id = id.to_string();
        tokio::spawn(async move { session.execute(id, execution).await });
        Some(done)
    }

    pub fn active_ids(&self) -> Vec<String> {
        self.active.lock().unwrap().keys().cloned().collect()
    }
    pub async fn pause_all(&self) {
        for id in self.active_ids() {
            self.pause(&id).await;
        }
    }
    pub async fn pause(&self, id: &str) {
        self.cancel(id, JobState::Paused).await
    }
    pub async fn stop(&self, id: &str) {
        self.cancel(id, JobState::Stopped).await
    }
    pub async fn stop_all(&self) {
        let ids: Vec<String> = self.active.lock().unwrap().keys().cloned().collect();
        for id in ids {
            self.stop(&id).await;
        }
    }

    async fn cancel(&self, id: &str, state: JobState) {
        let execution = self.active.lock().unwrap().get(id).cloned();
        let Some(execution) = execution else {
            // Not running: a queued, paused or stopped row just changes label.
            self.engine.queue.mutate(id, |job| {
                if matches!(job.state, JobState::Queued | JobState::Paused | JobState::Stopped) {
                    job.state = state;
                    job.detail = cancellation_detail(state);
                }
            });
            return;
        };
        *execution.cancel_state.lock().unwrap() = state;
        self.engine.queue.mutate(id, |job| job.is_stopping = true);
        execution.ct.cancel();
        wait_done(execution.done.subscribe()).await;
    }

    async fn execute(self: Arc<Self>, id: String, execution: Arc<Execution>) {
        let queue = self.engine.queue.clone();
        let mut attempt = 0u32;
        let result = loop {
            let permit = tokio::select! {
                _ = execution.ct.cancelled() => None,
                permit = self.slots.clone().acquire_owned() => permit.ok(),
            };
            let outcome = match permit {
                None => Err(Error::Cancelled),
                Some(_permit) => {
                    if queue.read(&id, |j| j.is_removing).unwrap_or(true) {
                        self.engine.set(&id, JobState::Stopped, Msg::plain("Đã dừng"));
                        Ok(())
                    } else if execution.ct.is_cancelled() {
                        Err(Error::Cancelled)
                    } else {
                        self.engine.process(&id, &self.root, self.flatten, &self.allocator, &execution.ct).await
                    }
                }
            };
            // A dropped connection, a timeout or a dead download link: wait (without holding a slot, so other jobs
            // keep going) and run the job again. Finished files are skipped and partial ones resume from their bytes.
            match outcome {
                Err(error) if error.is_transient() && !execution.ct.is_cancelled() && attempt < AUTO_RETRIES => {
                    attempt += 1;
                    let wait = (self.engine.timing.job_retry * 2u32.pow(attempt - 1)).min(MAX_RETRY_WAIT);
                    queue.mutate(&id, |job| {
                        job.state = JobState::Waiting;
                        job.detail = Msg::new("Lỗi kết nối · tự thử lại sau {0} giây ({1}/{2})…", &[&wait.as_secs(), &attempt, &AUTO_RETRIES]);
                    });
                    if pause(&execution.ct, wait).await.is_err() {
                        break Err(Error::Cancelled);
                    }
                }
                other => break other,
            }
        };
        match result {
            Ok(()) => {}
            Err(Error::Cancelled) if execution.ct.is_cancelled() => {
                let state = *execution.cancel_state.lock().unwrap();
                queue.mutate(&id, |job| {
                    job.state = state;
                    job.detail = cancellation_detail(state);
                });
            }
            Err(error) => {
                queue.mutate(&id, |job| {
                    job.state = JobState::Failed;
                    job.detail = error.friendly();
                });
            }
        }
        {
            let mut active = self.active.lock().unwrap();
            active.remove(&id);
            if active.is_empty() {
                self.idle.send_replace(true);
            }
        }
        // Clear the flags and publish before callers awaiting Stop can remove the row.
        queue.mutate(&id, |job| {
            job.is_active = false;
            job.is_stopping = false;
        });
        execution.done.send_replace(true);
    }
}
