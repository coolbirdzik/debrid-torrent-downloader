//! Video thumbnails for the Library.
//!
//! The webview grabs one frame with a hidden `<video>` and a canvas, so the bytes it reads must come from a CORS-enabled
//! origin or the canvas is tainted. Every video byte therefore goes through the `thumb` URI scheme served here:
//! a downloaded file is read from disk, a cloud file is proxied from its service link. Each source sits behind an opaque
//! token handed out by `register`, so the scheme never reads a path or fetches a URL the page names itself.
//!
//! Responses to `Range: bytes=N-` are always capped at `CHUNK` bytes (206). Chromium asks again for the next range,
//! so a multi-GB file is never pulled into memory.
//!
//! Finished thumbnails (and the files that failed) are cached in `<data>/thumbnails`, keyed by a hash of
//! provider + torrent + file ID, so each video costs at most one service link, ever.

use rdbatch_core::models::TorrentInfo;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::http::{header, Method, Request, Response, StatusCode};

pub const SCHEME: &str = "thumb";
const CHUNK: u64 = 3 * 1024 * 1024;
/// A torrent's file list is reused for this long, so a folder of videos costs one `info` call, not one per file.
const INFO_TTL: Duration = Duration::from_secs(120);
/// Largest thumbnail accepted from the page; real ones are ~20 KB.
const MAX_IMAGE: usize = 512 * 1024;

#[derive(Clone)]
pub enum Source {
    Local(PathBuf),
    Remote(String),
}

pub struct Thumbs {
    dir: PathBuf,
    /// Thumbnail ID → image hash, or `None` for a video that could not be decoded.
    index: Mutex<HashMap<String, Option<String>>>,
    sources: Mutex<HashMap<String, Source>>,
    infos: Mutex<HashMap<String, (Instant, TorrentInfo)>>,
    http: reqwest::Client,
}

pub fn hash(id: &str) -> String {
    let digest = Sha256::digest(id.as_bytes());
    digest.iter().take(20).map(|b| format!("{b:02x}")).collect()
}

impl Thumbs {
    pub fn new(data_dir: &Path) -> Thumbs {
        let dir = data_dir.join("thumbnails");
        let index = std::fs::read(dir.join("index.json")).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .user_agent("DebridDock")
            .build()
            .expect("http client");
        Thumbs { dir, index: Mutex::new(index), sources: Mutex::new(HashMap::new()), infos: Mutex::new(HashMap::new()), http }
    }

    pub fn known(&self) -> HashMap<String, Option<String>> {
        self.index.lock().unwrap().clone()
    }

    pub fn register(&self, source: Source) -> String {
        let token = uuid_token();
        self.sources.lock().unwrap().insert(token.clone(), source);
        token
    }

    pub fn release(&self, token: &str) {
        self.sources.lock().unwrap().remove(token);
    }

    pub fn cached_info(&self, key: &str) -> Option<TorrentInfo> {
        let infos = self.infos.lock().unwrap();
        infos.get(key).filter(|(at, _)| at.elapsed() < INFO_TTL).map(|(_, info)| info.clone())
    }

    pub fn remember_info(&self, key: String, info: TorrentInfo) {
        let mut infos = self.infos.lock().unwrap();
        infos.retain(|_, (at, _)| at.elapsed() < INFO_TTL);
        infos.insert(key, (Instant::now(), info));
    }

    /// Stores a JPEG for `id`, or remembers that the video cannot give one (`jpeg` = None). Returns the image hash.
    pub fn save(&self, id: &str, jpeg: Option<Vec<u8>>) -> Result<Option<String>, String> {
        let entry = match jpeg {
            Some(bytes) => {
                if bytes.len() > MAX_IMAGE || !bytes.starts_with(&[0xFF, 0xD8]) {
                    return Err("invalid thumbnail".into());
                }
                let name = hash(id);
                std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
                std::fs::write(self.dir.join(format!("{name}.jpg")), bytes).map_err(|e| e.to_string())?;
                Some(name)
            }
            None => None,
        };
        let snapshot = {
            let mut index = self.index.lock().unwrap();
            index.insert(id.to_string(), entry.clone());
            serde_json::to_vec(&*index).map_err(|e| e.to_string())?
        };
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let temp = self.dir.join("index.json.tmp");
        std::fs::write(&temp, snapshot).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, self.dir.join("index.json")).map_err(|e| e.to_string())?;
        Ok(entry)
    }

    /// Handles one request of the `thumb` scheme: `/img-<hash>` for a cached thumbnail, `/src-<token>` for video bytes.
    pub async fn serve(&self, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
        if request.method() == Method::OPTIONS {
            return respond(StatusCode::NO_CONTENT).body(Vec::new()).unwrap();
        }
        let path = request.uri().path().trim_start_matches('/');
        if let Some(name) = path.strip_prefix("img-") {
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_hexdigit()) {
                return status(StatusCode::BAD_REQUEST);
            }
            return match std::fs::read(self.dir.join(format!("{name}.jpg"))) {
                Ok(bytes) => respond(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/jpeg")
                    .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
                    .body(bytes)
                    .unwrap(),
                Err(_) => status(StatusCode::NOT_FOUND),
            };
        }
        let Some(source) = path.strip_prefix("src-").and_then(|token| self.sources.lock().unwrap().get(token).cloned()) else {
            return status(StatusCode::NOT_FOUND);
        };
        let range = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(parse_range).unwrap_or((0, None));
        match source {
            Source::Local(path) => tauri::async_runtime::spawn_blocking(move || serve_local(&path, range)).await.unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
            Source::Remote(url) => self.serve_remote(&url, range).await,
        }
    }

    async fn serve_remote(&self, url: &str, (start, end): (u64, Option<u64>)) -> Response<Vec<u8>> {
        let last = end.map_or(start + CHUNK - 1, |end| end.min(start + CHUNK - 1));
        let Ok(mut upstream) = self.http.get(url).header(header::RANGE.as_str(), format!("bytes={start}-{last}")).send().await else {
            return status(StatusCode::BAD_GATEWAY);
        };
        let partial = upstream.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if !(partial || (upstream.status().is_success() && start == 0)) {
            return status(if upstream.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE { StatusCode::RANGE_NOT_SATISFIABLE } else { StatusCode::BAD_GATEWAY });
        }
        let total = if partial {
            upstream.headers().get(reqwest::header::CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(|v| v.rsplit('/').next()).and_then(|v| v.parse::<u64>().ok())
        } else {
            upstream.content_length()
        };
        let Some(total) = total else { return status(StatusCode::BAD_GATEWAY) };
        let content_type = upstream.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream").to_string();
        // A server that ignored the range sends the whole file: keep only the first chunk of it.
        let want = (last - start + 1) as usize;
        let mut body = Vec::with_capacity(want.min(CHUNK as usize));
        while body.len() < want {
            match upstream.chunk().await {
                Ok(Some(bytes)) => body.extend_from_slice(&bytes),
                Ok(None) => break,
                Err(_) => return status(StatusCode::BAD_GATEWAY),
            }
        }
        body.truncate(want);
        if body.is_empty() {
            return status(StatusCode::RANGE_NOT_SATISFIABLE);
        }
        partial_response(start, body, total, &content_type)
    }
}

fn serve_local(path: &Path, (start, end): (u64, Option<u64>)) -> Response<Vec<u8>> {
    let Ok(mut file) = std::fs::File::open(path) else { return status(StatusCode::NOT_FOUND) };
    let Ok(total) = file.metadata().map(|m| m.len()) else { return status(StatusCode::NOT_FOUND) };
    if start >= total {
        return respond(StatusCode::RANGE_NOT_SATISFIABLE).header(header::CONTENT_RANGE, format!("bytes */{total}")).body(Vec::new()).unwrap();
    }
    let last = end.unwrap_or(u64::MAX).min(start + CHUNK - 1).min(total - 1);
    let mut body = vec![0; (last - start + 1) as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut body).is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    partial_response(start, body, total, local_type(path))
}

fn partial_response(start: u64, body: Vec<u8>, total: u64, content_type: &str) -> Response<Vec<u8>> {
    let last = start + body.len() as u64 - 1;
    respond(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_RANGE, format!("bytes {start}-{last}/{total}"))
        .header(header::CONTENT_LENGTH, body.len())
        .body(body)
        .unwrap()
}

fn local_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("ogv") => "video/ogg",
        _ => "application/octet-stream",
    }
}

fn respond(code: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(code)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range")
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, OPTIONS")
        .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "Content-Range, Content-Length, Accept-Ranges")
        .header(header::ACCEPT_RANGES, "bytes")
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    respond(code).body(Vec::new()).unwrap()
}

/// `bytes=N-` or `bytes=N-M`; suffix ranges (`bytes=-N`) and multiple ranges are not used by media elements.
fn parse_range(value: &str) -> Option<(u64, Option<u64>)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split(',').next()?.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let end = match end.trim() {
        "" => None,
        end => Some(end.parse().ok().filter(|&e| e >= start)?),
    };
    Some((start, end))
}

fn uuid_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seed = format!("{:?}-{}-{}", Instant::now(), COUNTER.fetch_add(1, Ordering::Relaxed), std::process::id());
    hash(&seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_ranges() {
        assert_eq!(parse_range("bytes=0-"), Some((0, None)));
        assert_eq!(parse_range("bytes=100-199"), Some((100, Some(199))));
        assert_eq!(parse_range("bytes=200-100"), None);
        assert_eq!(parse_range("bytes=-500"), None);
        assert_eq!(parse_range("items=0-1"), None);
    }

    #[test]
    fn local_ranges_are_capped_and_described() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        std::fs::write(&path, vec![7u8; (CHUNK + 10) as usize]).unwrap();
        let first = serve_local(&path, (0, None));
        assert_eq!(first.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(first.body().len() as u64, CHUNK);
        assert_eq!(first.headers()[header::CONTENT_RANGE], format!("bytes 0-{}/{}", CHUNK - 1, CHUNK + 10).as_str());
        let tail = serve_local(&path, (CHUNK, None));
        assert_eq!(tail.body().len(), 10);
        assert_eq!(serve_local(&path, (CHUNK + 10, None)).status(), StatusCode::RANGE_NOT_SATISFIABLE);
    }

    #[test]
    fn saves_images_and_failures_in_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = Thumbs::new(dir.path());
        let name = thumbs.save("realDebrid|abc|1", Some(vec![0xFF, 0xD8, 1, 2])).unwrap().unwrap();
        assert!(dir.path().join("thumbnails").join(format!("{name}.jpg")).is_file());
        assert_eq!(thumbs.save("realDebrid|abc|2", None).unwrap(), None);
        assert!(thumbs.save("realDebrid|abc|3", Some(b"not a jpeg".to_vec())).is_err());
        let reloaded = Thumbs::new(dir.path()).known();
        assert_eq!(reloaded.get("realDebrid|abc|1"), Some(&Some(name)));
        assert_eq!(reloaded.get("realDebrid|abc|2"), Some(&None));
    }
}
