use super::http::retry_after;
use super::{Debrid, Timing};
use crate::error::{Error, Result};
use crate::models::{Msg, Provider, RemoteTorrent, TorrentInfo, Unrestricted, User};
use async_trait::async_trait;
use reqwest::{Method, RequestBuilder};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use url::Url;

pub struct RealDebrid {
    http: reqwest::Client,
    token: String,
    base: Url,
    spacing: Duration,
    retry_base: Duration,
    next: Mutex<Instant>,
}

impl RealDebrid {
    pub fn new(http: reqwest::Client, token: &str, base_url: Option<&str>, timing: Timing) -> Self {
        let mut base = base_url.unwrap_or("https://api.real-debrid.com/rest/1.0/").to_string();
        if !base.ends_with('/') {
            base.push('/');
        }
        RealDebrid {
            http,
            token: token.to_string(),
            base: Url::parse(&base).expect("Real-Debrid base URL"),
            spacing: timing.gate_realdebrid,
            retry_base: timing.retry_base,
            next: Mutex::new(Instant::now()),
        }
    }

    fn redact(&self, value: &str) -> String {
        if self.token.is_empty() { value.to_string() } else { value.replace(&self.token, "[redacted]") }
    }

    /// One shared limiter per client, including account checks. Only safe reads and explicit rate limits are retried;
    /// an ambiguous upload or delete is never replayed.
    async fn request(&self, method: Method, path: &str, build: &(dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync)) -> Result<Option<Value>> {
        let url = self.base.join(path).map_err(|e| Error::invalid(e.to_string()))?;
        let mut attempt = 0u32;
        loop {
            {
                let mut next = self.next.lock().await;
                let now = Instant::now();
                if *next > now {
                    tokio::time::sleep(*next - now).await;
                }
                *next = Instant::now() + self.spacing;
            }
            let response = build(self.http.request(method.clone(), url.clone()).bearer_auth(&self.token)).send().await?;
            let status = response.status();
            if attempt < 3 && (status.as_u16() == 429 || (method == Method::GET && status.as_u16() >= 500)) {
                let wait = retry_after(&response).unwrap_or(self.retry_base * 2u32.pow(attempt));
                tokio::time::sleep(if wait.is_zero() { Duration::from_millis(5) } else { wait }).await;
                attempt += 1;
                continue;
            }
            let body = response.text().await?;
            if !status.is_success() {
                let mut error = "Yêu cầu thất bại".to_string();
                let mut code = None;
                if let Ok(value) = serde_json::from_str::<Value>(&body) {
                    if let Some(text) = value.get("error").and_then(Value::as_str) {
                        error = text.to_string();
                    }
                    code = value.get("error_code").and_then(Value::as_i64);
                }
                let suffix = code.map(|c| format!(" (mã {c})")).unwrap_or_default();
                return Err(Error::Api {
                    provider: Provider::RealDebrid,
                    status: status.as_u16(),
                    code: code.map(|c| c.to_string()).unwrap_or_default(),
                    message: self.redact(&format!("Real-Debrid {}: {}{}", status.as_u16(), error, suffix)),
                    error_code: code,
                });
            }
            if body.trim().is_empty() {
                return Ok(None);
            }
            return Ok(Some(serde_json::from_str(&body)?));
        }
    }

    async fn typed<T: serde::de::DeserializeOwned>(&self, method: Method, path: &str, build: &(dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync)) -> Result<T> {
        match self.request(method, path, build).await? {
            Some(value) => Ok(serde_json::from_value(value)?),
            None => Err(Error::Operation(Msg::plain("Real-Debrid trả về dữ liệu trống."))),
        }
    }
}

fn escape(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[async_trait]
impl Debrid for RealDebrid {
    fn provider(&self) -> Provider {
        Provider::RealDebrid
    }

    async fn user(&self) -> Result<User> {
        let value: Value = self.typed(Method::GET, "user", &|r| r).await?;
        Ok(User {
            username: value.get("username").and_then(Value::as_str).unwrap_or_default().to_string(),
            kind: value.get("type").and_then(Value::as_str).unwrap_or_default().to_string(),
            premium: value.get("premium").and_then(Value::as_i64).unwrap_or(0),
            allows_downloads: false,
        })
    }

    async fn info(&self, id: &str) -> Result<TorrentInfo> {
        self.typed(Method::GET, &format!("torrents/info/{}", escape(id)), &|r| r).await
    }

    /// Every torrent on the token's account, page by page.
    async fn list(&self) -> Result<Vec<RemoteTorrent>> {
        const PAGE: usize = 1000;
        let mut all: Vec<RemoteTorrent> = Vec::new();
        for page in 1..=100 {
            // An empty account may answer with no body at all.
            let batch: Vec<RemoteTorrent> = match self.request(Method::GET, &format!("torrents?page={page}&limit={PAGE}"), &|r| r).await? {
                Some(value) => serde_json::from_value(value)?,
                None => Vec::new(),
            };
            if batch.is_empty() {
                break;
            }
            let count = batch.len();
            all.extend(batch);
            if count < PAGE {
                break;
            }
        }
        let mut seen = std::collections::HashSet::new();
        all.retain(|torrent| seen.insert(torrent.id.clone()));
        Ok(all)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        if id.trim().is_empty() {
            return Err(Error::invalid("Torrent ID is required."));
        }
        self.request(Method::DELETE, &format!("torrents/delete/{}", escape(id)), &|r| r).await?;
        Ok(())
    }

    async fn add_magnet(&self, magnet: &str) -> Result<String> {
        let value: Value = self.typed(Method::POST, "torrents/addMagnet", &|r| r.form(&[("magnet", magnet)])).await?;
        Ok(value.get("id").and_then(Value::as_str).unwrap_or_default().to_string())
    }

    async fn add_torrent(&self, bytes: Vec<u8>) -> Result<String> {
        let value: Value = self.typed(Method::PUT, "torrents/addTorrent", &|r| r.body(bytes.clone())).await?;
        Ok(value.get("id").and_then(Value::as_str).unwrap_or_default().to_string())
    }

    async fn select_all(&self, id: &str) -> Result<()> {
        self.request(Method::POST, &format!("torrents/selectFiles/{}", escape(id)), &|r| r.form(&[("files", "all")])).await?;
        Ok(())
    }

    async fn select_files(&self, id: &str, file_ids: &[i32]) -> Result<()> {
        let mut ids: Vec<i32> = file_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() || ids.iter().any(|value| *value <= 0) {
            return Err(Error::Operation(Msg::plain("Chọn ít nhất một file để tải.")));
        }
        let joined = ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
        self.request(Method::POST, &format!("torrents/selectFiles/{}", escape(id)), &|r| r.form(&[("files", joined.as_str())])).await?;
        Ok(())
    }

    async fn unrestrict(&self, link: &str) -> Result<Unrestricted> {
        let value: Value = self.typed(Method::POST, "unrestrict/link", &|r| r.form(&[("link", link)])).await?;
        Ok(Unrestricted {
            filename: value.get("filename").and_then(Value::as_str).unwrap_or_default().to_string(),
            filesize: value.get("filesize").and_then(Value::as_u64).unwrap_or(0),
            download: value.get("download").and_then(Value::as_str).unwrap_or_default().to_string(),
        })
    }
}
