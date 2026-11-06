//! Resumable single-file downloads: `<name>.part` plus `<name>.part.json` hold the partial bytes and the validator
//! that lets `Range` + `If-Range` continue them. A completed file is never overwritten.

use crate::error::{Error, Result};
use crate::models::Msg;
use crate::paths;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransferProgress {
    pub bytes: u64,
    pub total: u64,
    pub bytes_per_second: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartialMetadata {
    expected_bytes: u64,
    validator: Option<String>,
}

const HEADER_TIMEOUT: Duration = Duration::from_secs(90);
const READ_TIMEOUT: Duration = Duration::from_secs(90);
const REPORT_EVERY: Duration = Duration::from_millis(250);

fn partial_path(destination: &Path) -> PathBuf {
    let mut name = destination.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

fn metadata_path(destination: &Path) -> PathBuf {
    let mut name = partial_path(destination).into_os_string();
    name.push(".json");
    PathBuf::from(name)
}

fn operation(key: &str) -> Error {
    Error::Operation(Msg::plain(key))
}

/// Parse `bytes <from>-<to>/<length>` or `bytes */<length>`.
fn parse_content_range(value: &str) -> Option<(Option<(u64, u64)>, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim();
    let (range, length) = rest.split_once('/')?;
    let length = length.trim().parse::<u64>().ok();
    let range = range.trim();
    if range == "*" {
        return Some((None, length));
    }
    let (from, to) = range.split_once('-')?;
    Some((Some((from.trim().parse().ok()?, to.trim().parse().ok()?)), length))
}

pub async fn download(
    http: &reqwest::Client,
    url: &str,
    destination: &Path,
    expected_bytes: u64,
    progress: &(dyn Fn(TransferProgress) + Send + Sync),
    ct: &CancellationToken,
) -> Result<()> {
    match url::Url::parse(url) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {}
        _ => return Err(operation("Link tải không hợp lệ.")),
    }
    let name = destination.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if destination.exists() {
        return Err(Error::Operation(Msg::new("File đích đã tồn tại; app không ghi đè: {0}", &[&name])));
    }
    let directory = destination.parent().ok_or_else(|| Error::invalid("Missing download directory"))?;
    tokio::fs::create_dir_all(directory).await?;
    let partial = partial_path(destination);
    let metadata = metadata_path(destination);
    paths::ensure_no_reparse_points(directory, &partial)?;
    paths::ensure_no_reparse_points(directory, &metadata)?;

    let saved: Option<PartialMetadata> = match tokio::fs::read_to_string(&metadata).await {
        Ok(text) => serde_json::from_str(&text).ok(),
        Err(_) => None,
    };
    let mut offset = match (tokio::fs::metadata(&partial).await, &saved) {
        (Ok(meta), Some(saved)) if saved.expected_bytes == expected_bytes => meta.len(),
        _ => 0,
    };
    if expected_bytes > 0 && offset > expected_bytes {
        offset = 0;
    }

    let mut request = http.get(url).header(ACCEPT_ENCODING, "identity");
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
        if let Some(validator) = saved.as_ref().and_then(|s| s.validator.as_ref()).filter(|v| !v.is_empty()) {
            request = request.header(IF_RANGE, validator);
        }
    }
    let response = tokio::select! {
        _ = ct.cancelled() => return Err(Error::Cancelled),
        sent = tokio::time::timeout(HEADER_TIMEOUT, request.send()) => sent.map_err(|_| Error::Timeout)??,
    };
    let status = response.status().as_u16();

    // The partial file may already be complete: the server answers "range not satisfiable" with the full length.
    if status == 416 && offset > 0 {
        let length = response.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(parse_content_range).and_then(|(_, l)| l);
        if length == Some(offset) && (expected_bytes == 0 || offset == expected_bytes) {
            finish(&partial, &metadata, destination).await?;
            progress(TransferProgress { bytes: offset, total: offset, bytes_per_second: 0.0 });
            return Ok(());
        }
    }
    if !response.status().is_success() {
        return Err(Error::Network(format!("HTTP {status}")));
    }

    let total;
    if status == 206 {
        let range = response.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(parse_content_range);
        match range {
            Some((Some((from, to)), Some(length))) if from == offset && to + 1 == length => total = length,
            _ => return Err(operation("Máy chủ trả về khoảng dữ liệu không hợp lệ; chưa ghép file.")),
        }
    } else {
        // The server ignored Range, or If-Range detected a changed file: start cleanly instead of appending.
        offset = 0;
        total = response.headers().get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(expected_bytes);
    }
    if expected_bytes > 0 && total > 0 && total != expected_bytes {
        return Err(operation("Kích thước link tải khác thông tin dịch vụ; chưa lưu file hoàn tất."));
    }
    let total = if total == 0 { expected_bytes } else { total };
    let validator = match response.headers().get(ETAG).and_then(|v| v.to_str().ok()) {
        Some(tag) if !tag.starts_with("W/") => Some(tag.to_string()),
        _ => response.headers().get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(str::to_string),
    };
    tokio::fs::write(&metadata, serde_json::to_vec(&PartialMetadata { expected_bytes, validator })?).await?;

    let mut file = if offset > 0 {
        tokio::fs::OpenOptions::new().write(true).open(&partial).await?
    } else {
        tokio::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&partial).await?
    };
    file.seek(std::io::SeekFrom::Start(offset)).await?;

    let mut downloaded = offset;
    let started = Instant::now();
    let mut last_report = Duration::ZERO;
    let mut stream = response.bytes_stream();
    progress(TransferProgress { bytes: downloaded, total, bytes_per_second: 0.0 });
    let outcome: Result<()> = async {
        loop {
            let next = tokio::select! {
                biased;
                _ = ct.cancelled() => return Err(Error::Cancelled),
                next = tokio::time::timeout(READ_TIMEOUT, stream.next()) => next.map_err(|_| Error::Timeout)?,
            };
            let Some(chunk) = next else { break };
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if started.elapsed() - last_report > REPORT_EVERY {
                let speed = (downloaded - offset) as f64 / started.elapsed().as_secs_f64().max(0.001);
                progress(TransferProgress { bytes: downloaded, total, bytes_per_second: speed });
                last_report = started.elapsed();
            }
        }
        file.flush().await?;
        if total > 0 && downloaded != total {
            // A connection that closed early: transient, so the engine retries and resumes from these bytes.
            return Err(Error::Network(format!("short body: {downloaded} of {total} bytes")));
        }
        Ok(())
    }
    .await;
    // Always report the final byte count, so a stopped job shows exactly what is on disk.
    let _ = file.flush().await;
    drop(file);
    progress(TransferProgress { bytes: downloaded, total: if total > 0 { total } else { downloaded }, bytes_per_second: 0.0 });
    outcome?;
    finish(&partial, &metadata, destination).await
}

/// Move the finished partial into place, never over an existing file.
async fn finish(partial: &Path, metadata: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        let name = destination.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        return Err(Error::Operation(Msg::new("File đích đã tồn tại; app không ghi đè: {0}", &[&name])));
    }
    tokio::fs::rename(partial, destination).await?;
    let _ = tokio::fs::remove_file(metadata).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn collect() -> (Arc<Mutex<Vec<TransferProgress>>>, impl Fn(TransferProgress) + Send + Sync) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (seen, move |p| sink.lock().unwrap().push(p))
    }

    const BODY: &[u8] = b"0123456789";

    #[tokio::test]
    async fn fresh_download_verifies_bytes_and_never_overwrites() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/f.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(BODY)).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("f.bin");
        let (seen, progress) = collect();
        download(&reqwest::Client::new(), &format!("{}/f.bin", server.uri()), &destination, 10, &progress, &CancellationToken::new()).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), BODY);
        assert!(!partial_path(&destination).exists() && !metadata_path(&destination).exists());
        assert_eq!(seen.lock().unwrap().last().unwrap().bytes, 10);
        // A second run must refuse to replace the finished file.
        let again = download(&reqwest::Client::new(), &format!("{}/f.bin", server.uri()), &destination, 10, &progress, &CancellationToken::new()).await;
        assert!(matches!(again, Err(Error::Operation(_))));
        assert_eq!(std::fs::read(&destination).unwrap(), BODY);
    }

    #[tokio::test]
    async fn resume_uses_range_and_if_range_and_joins_exact_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/f.bin"))
            .and(header("range", "bytes=4-"))
            .and(header("if-range", "\"v1\""))
            .respond_with(ResponseTemplate::new(206).insert_header("content-range", "bytes 4-9/10").set_body_bytes(&BODY[4..]))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("f.bin");
        std::fs::write(partial_path(&destination), &BODY[..4]).unwrap();
        std::fs::write(metadata_path(&destination), br#"{"expectedBytes":10,"validator":"\"v1\""}"#).unwrap();
        let (_, progress) = collect();
        download(&reqwest::Client::new(), &format!("{}/f.bin", server.uri()), &destination, 10, &progress, &CancellationToken::new()).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), BODY);
    }

    #[tokio::test]
    async fn server_ignoring_range_restarts_without_appending() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/f.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(BODY)).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("f.bin");
        std::fs::write(partial_path(&destination), b"WRONG").unwrap();
        std::fs::write(metadata_path(&destination), br#"{"expectedBytes":10,"validator":null}"#).unwrap();
        let (_, progress) = collect();
        download(&reqwest::Client::new(), &format!("{}/f.bin", server.uri()), &destination, 10, &progress, &CancellationToken::new()).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), BODY);
    }

    #[tokio::test]
    async fn wrong_content_range_and_truncated_bodies_are_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/range.bin"))
            .respond_with(ResponseTemplate::new(206).insert_header("content-range", "bytes 0-4/10").set_body_bytes(&BODY[..5]))
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/short.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(&BODY[..6])).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let (_, progress) = collect();
        // A partial file exists, so the downloader sends Range and the server answers from the wrong offset.
        let ranged = dir.path().join("range.bin");
        std::fs::write(partial_path(&ranged), &BODY[..2]).unwrap();
        std::fs::write(metadata_path(&ranged), br#"{"expectedBytes":10,"validator":null}"#).unwrap();
        let result = download(&reqwest::Client::new(), &format!("{}/range.bin", server.uri()), &ranged, 10, &progress, &CancellationToken::new()).await;
        assert!(result.is_err() && !ranged.exists());
        // The server says 6 bytes but 10 were expected: no completed file is saved.
        let short = dir.path().join("short.bin");
        let result = download(&reqwest::Client::new(), &format!("{}/short.bin", server.uri()), &short, 10, &progress, &CancellationToken::new()).await;
        assert!(result.is_err() && !short.exists());
    }

    /// A raw server that sends part of the body and then stalls, to cancel mid-transfer.
    async fn stalling_server(sent: &'static [u8], length: usize) -> String {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                tokio::spawn(async move {
                    let mut buffer = [0u8; 2048];
                    let _ = socket.read(&mut buffer).await;
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(sent).await;
                    let _ = socket.flush().await;
                    tokio::time::sleep(Duration::from_secs(30)).await;
                });
            }
        });
        format!("http://{address}/slow.bin")
    }

    #[tokio::test]
    async fn cancellation_preserves_the_partial_download() {
        let url = stalling_server(b"01234", 10).await;
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("slow.bin");
        let ct = CancellationToken::new();
        let (_, progress) = collect();
        let canceller = ct.clone();
        tokio::spawn(async move {
            // The server sends five bytes at once; give the downloader time to write them, then cancel.
            tokio::time::sleep(Duration::from_millis(400)).await;
            canceller.cancel();
        });
        let result = download(&reqwest::Client::new(), &url, &destination, 10, &progress, &ct).await;
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!destination.exists());
        assert_eq!(std::fs::read(partial_path(&destination)).unwrap(), b"01234");
        assert!(metadata_path(&destination).exists(), "the validator must survive so the next run can resume");
    }

    #[test]
    fn content_range_parsing() {
        assert_eq!(parse_content_range("bytes 4-9/10"), Some((Some((4, 9)), Some(10))));
        assert_eq!(parse_content_range("bytes */10"), Some((None, Some(10))));
        assert_eq!(parse_content_range("bytes 4-9/*"), Some((Some((4, 9)), None)));
        assert_eq!(parse_content_range("items 1-2/3"), None);
    }
}
