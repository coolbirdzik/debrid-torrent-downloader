use super::http::{date, items, num, prop, str_of, flag, ProviderHttp};
use super::{Debrid, Timing};
use crate::error::{Error, Result};
use crate::models::{Msg, Provider, RemoteTorrent, TorrentFile, TorrentInfo, Unrestricted, User};
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use reqwest::Method;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::Duration;

pub struct AllDebrid {
    api: ProviderHttp,
    poll: Duration,
}

impl AllDebrid {
    pub fn new(http: reqwest::Client, token: &str, base_url: Option<&str>, timing: Timing) -> Self {
        AllDebrid { api: ProviderHttp::new(http, token, Provider::AllDebrid, "https://api.alldebrid.com/", base_url, timing), poll: timing.poll }
    }

    async fn magnets(&self, id: Option<&str>) -> Result<Vec<Value>> {
        let data = self
            .api
            .send(Method::POST, "v4.1/magnet/status", &|r| match id {
                Some(id) => r.form(&[("id", id)]),
                None => r.form::<[(&str, &str); 0]>(&[]),
            }, true)
            .await?;
        Ok(items(prop(&data, "magnets")).into_iter().cloned().collect())
    }
}

fn created_id(entries: &Value) -> Result<String> {
    let all = items(entries);
    let item = all.first().copied().cloned().unwrap_or(Value::Null);
    let error = prop(&item, "error");
    if error.is_object() {
        return Err(Error::Operation(Msg::plain(&format!("AllDebrid: {}", str_of(error, "message")))));
    }
    let id = str_of(&item, "id");
    if id.is_empty() { Err(Error::invalid("AllDebrid: missing magnet ID.")) } else { Ok(id) }
}

fn flatten(tree: &Value, parent: &str, entries: &mut Vec<(TorrentFile, String)>) {
    for item in items(tree) {
        let path = format!("{parent}/{}", str_of(item, "n"));
        if prop(item, "e").is_array() {
            flatten(prop(item, "e"), &path, entries);
            continue;
        }
        let link = str_of(item, "l");
        // Paths, rather than array indices, keep selection stable if the server reorders files.
        let hash = Sha256::digest(path.as_bytes());
        let id = (u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]) & 0x7fff_ffff) as i32;
        entries.push((TorrentFile { id: if id == 0 { 1 } else { id }, path, bytes: num(item, "s").max(0) as u64, selected: 1 }, link));
    }
}

fn state(item: &Value) -> String {
    match num(item, "statusCode") {
        0 => "queued",
        1 => "downloading",
        2 => "compressing",
        3 => "uploading",
        4 => "downloaded",
        _ => "error",
    }
    .into()
}

fn progress(item: &Value) -> f64 {
    if num(item, "statusCode") == 4 {
        100.0
    } else if num(item, "size") > 0 {
        (num(item, "downloaded") as f64 * 100.0 / num(item, "size") as f64).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

#[async_trait]
impl Debrid for AllDebrid {
    fn provider(&self) -> Provider {
        Provider::AllDebrid
    }

    async fn user(&self) -> Result<User> {
        let data = self.api.send(Method::GET, "v4/user", &|r| r, true).await?;
        let user = prop(&data, "user");
        let premium = flag(user, "isPremium") || flag(user, "isTrial");
        let name = str_of(user, "username");
        if name.is_empty() {
            return Err(Error::invalid("AllDebrid: missing account name."));
        }
        Ok(User { username: name, kind: if premium { "premium" } else { "free" }.into(), premium: i64::from(premium), allows_downloads: false, ..User::default() })
    }

    async fn add_magnet(&self, magnet: &str) -> Result<String> {
        let data = self.api.send(Method::POST, "v4/magnet/upload", &|r| r.form(&[("magnets[]", magnet)]), false).await?;
        created_id(prop(&data, "magnets"))
    }

    async fn add_torrent(&self, bytes: Vec<u8>) -> Result<String> {
        let data = self
            .api
            .send(Method::POST, "v4/magnet/upload/file", &|r| {
                r.multipart(Form::new().part("files[]", Part::bytes(bytes.clone()).file_name("upload.torrent")))
            }, false)
            .await?;
        created_id(prop(&data, "files"))
    }

    async fn info(&self, id: &str) -> Result<TorrentInfo> {
        let magnets = self.magnets(Some(id)).await?;
        let Some(item) = magnets.iter().find(|item| str_of(item, "id") == id) else {
            return Err(Error::Api {
                provider: Provider::AllDebrid,
                status: 404,
                code: "MAGNET_INVALID_ID".into(),
                message: "AllDebrid: MAGNET_INVALID_ID · Magnet not found.".into(),
                error_code: None,
            });
        };
        let mut tree = prop(item, "files").clone();
        if !tree.is_array() && num(item, "statusCode") == 4 {
            let data = self.api.send(Method::POST, "v4/magnet/files", &|r| r.form(&[("id[]", id)]), true).await?;
            tree = items(prop(&data, "magnets")).into_iter().find(|magnet| str_of(magnet, "id") == id).map(|m| prop(m, "files").clone()).unwrap_or(Value::Null);
        }
        let mut entries = Vec::new();
        flatten(&tree, "", &mut entries);
        let mut unique: Vec<i32> = entries.iter().map(|(file, _)| file.id).collect();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != entries.len() {
            return Err(Error::invalid("AllDebrid: ambiguous file paths."));
        }
        Ok(TorrentInfo {
            id: id.to_string(),
            filename: str_of(item, "filename"),
            status: state(item),
            progress: progress(item),
            bytes: num(item, "size").max(0) as u64,
            files: entries.iter().map(|(file, _)| file.clone()).collect(),
            links: entries.iter().map(|(_, link)| link.clone()).collect(),
            seeders: Some(num(item, "seeders")),
            speed: Some(num(item, "downloadSpeed").max(0) as u64),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteTorrent>> {
        Ok(self
            .magnets(None)
            .await?
            .iter()
            .map(|item| RemoteTorrent {
                id: str_of(item, "id"),
                filename: str_of(item, "filename"),
                bytes: num(item, "size").max(0) as u64,
                progress: progress(item),
                status: state(item),
                added: date(item, "uploadDate"),
                speed: Some(num(item, "downloadSpeed").max(0) as u64),
                seeders: Some(num(item, "seeders")),
            })
            .collect())
    }

    async fn select_all(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn select_files(&self, _id: &str, _file_ids: &[i32]) -> Result<()> {
        Ok(())
    }

    async fn unrestrict(&self, link: &str) -> Result<Unrestricted> {
        if link.trim().is_empty() {
            return Err(Error::invalid("AllDebrid: file link is not ready."));
        }
        let data = self.api.send(Method::POST, "v4/link/unlock", &|r| r.form(&[("link", link)]), false).await?;
        let filename = str_of(&data, "filename");
        let size = num(&data, "filesize").max(0) as u64;
        let mut url = str_of(&data, "link");
        let delayed_id = str_of(&data, "delayed");
        // A delayed link is polled by its own ID: unlocking again would regenerate it.
        while url.is_empty() && !delayed_id.is_empty() {
            tokio::time::sleep(self.poll).await;
            let delayed = self.api.send(Method::POST, "v4/link/delayed", &|r| r.form(&[("id", delayed_id.as_str())]), true).await?;
            if num(&delayed, "status") == 3 {
                return Err(Error::Operation(Msg::plain("AllDebrid không tạo được link tải.")));
            }
            if num(&delayed, "status") == 2 {
                url = str_of(&delayed, "link");
            }
        }
        match url::Url::parse(&url) {
            Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => {}
            _ => return Err(Error::Operation(Msg::plain("AllDebrid chưa cung cấp link trực tiếp. Thử lại sau."))),
        }
        Ok(Unrestricted { filename, filesize: size, download: url })
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.api.send(Method::POST, "v4/magnet/delete", &|r| r.form(&[("id", id)]), false).await?;
        Ok(())
    }
}
