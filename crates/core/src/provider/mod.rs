//! Provider adapters normalize each service's metadata and restricted-file references so the engine and the local
//! downloader are shared. Base URLs and pacing are injectable so every adapter can be tested against a mock server.

mod alldebrid;
mod http;
mod realdebrid;
mod torbox;

pub use alldebrid::AllDebrid;
pub use realdebrid::RealDebrid;
pub use torbox::TorBox;

use crate::error::Result;
use crate::models::{CloudKind, Provider, RemoteTorrent, TorBoxAccount, TorrentInfo, Unrestricted, User};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Pacing and polling intervals. Production values stay well below each service's rate limit.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub gate_realdebrid: Duration,
    pub gate_other: Duration,
    /// Base wait before replaying a rate-limited or flaky safe read.
    pub retry_base: Duration,
    /// Poll interval while a torrent is being prepared remotely.
    pub poll: Duration,
    /// Poll interval while inspecting metadata.
    pub inspect_poll: Duration,
    /// Base wait before retrying a broken file transfer (multiplied by the attempt number).
    pub transfer_retry: Duration,
    /// First wait before a job that failed on a connection problem runs again on its own (doubles each time).
    pub job_retry: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            gate_realdebrid: Duration::from_millis(400),
            gate_other: Duration::from_millis(500),
            retry_base: Duration::from_secs(5),
            poll: Duration::from_secs(5),
            inspect_poll: Duration::from_secs(2),
            transfer_retry: Duration::from_secs(3),
            job_retry: Duration::from_secs(10),
        }
    }
}

impl Timing {
    /// Near-zero waits for tests.
    pub fn fast() -> Self {
        let tick = Duration::from_millis(5);
        Timing { gate_realdebrid: tick, gate_other: tick, retry_base: tick, poll: tick, inspect_poll: tick, transfer_retry: tick, job_retry: tick }
    }
}

#[async_trait]
pub trait Debrid: Send + Sync {
    fn provider(&self) -> Provider;
    async fn user(&self) -> Result<User>;
    async fn add_magnet(&self, magnet: &str) -> Result<String>;
    async fn add_torrent(&self, bytes: Vec<u8>) -> Result<String>;
    async fn info(&self, id: &str) -> Result<TorrentInfo>;
    async fn list(&self) -> Result<Vec<RemoteTorrent>>;
    async fn select_all(&self, id: &str) -> Result<()>;
    async fn select_files(&self, id: &str, file_ids: &[i32]) -> Result<()>;
    async fn unrestrict(&self, link: &str) -> Result<Unrestricted>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn cloud_list(&self, kind: CloudKind) -> Result<Vec<RemoteTorrent>> {
        if kind == CloudKind::Torrent { self.list().await }
        else { Err(crate::error::Error::invalid("Unsupported cloud download type.")) }
    }
    async fn cloud_delete(&self, kind: CloudKind, id: &str) -> Result<()> {
        if kind == CloudKind::Torrent { self.delete(id).await }
        else { Err(crate::error::Error::invalid("Unsupported cloud download type.")) }
    }
    async fn torbox_account(&self) -> Result<TorBoxAccount> {
        Err(crate::error::Error::invalid("TorBox account information is unavailable for this provider."))
    }
}

pub fn create(provider: Provider, http: reqwest::Client, token: &str, base_url: Option<&str>, timing: Timing) -> Arc<dyn Debrid> {
    match provider {
        Provider::RealDebrid => Arc::new(RealDebrid::new(http, token, base_url, timing)),
        Provider::TorBox => Arc::new(TorBox::new(http, token, base_url, timing)),
        Provider::AllDebrid => Arc::new(AllDebrid::new(http, token, base_url, timing)),
    }
}
