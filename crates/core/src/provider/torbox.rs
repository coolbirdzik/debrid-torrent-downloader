use super::http::{date, decimal, flag, items, num, prop, str_of, ProviderHttp};
use super::{Debrid, Timing};
use crate::error::{Error, Result};
use crate::models::{Msg, Provider, RemoteTorrent, TorrentFile, TorrentInfo, Unrestricted, User};
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use reqwest::Method;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct TorBox {
    api: ProviderHttp,
    token: String,
    files: Mutex<HashMap<String, TorrentFile>>,
}

fn escape(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

impl TorBox {
    pub fn new(http: reqwest::Client, token: &str, base_url: Option<&str>, timing: Timing) -> Self {
        TorBox {
            api: ProviderHttp::new(http, token, Provider::TorBox, "https://api.torbox.app/v1/api/", base_url, timing),
            token: token.to_string(),
            files: Mutex::new(HashMap::new()),
        }
    }

    async fn create(&self, build: impl Fn() -> Form + Send + Sync) -> Result<String> {
        let data = self
            .api
            .send(Method::POST, "torrents/createtorrent", &|r| {
                r.multipart(build().text("allow_zip", "false").text("as_queued", "false"))
            }, false)
            .await?;
        let id = str_of(&data, "torrent_id");
        if id.is_empty() {
            return Err(Error::invalid("TorBox: missing torrent ID."));
        }
        Ok(id)
    }
}

fn state(item: &Value) -> String {
    let download_state = str_of(item, "download_state");
    if flag(item, "download_finished") && flag(item, "download_present") {
        "downloaded".into()
    } else if download_state.to_lowercase().contains("error") {
        "error".into()
    } else if download_state == "metaDL" || download_state == "checkingResumeData" {
        "magnet_conversion".into()
    } else {
        "downloading".into()
    }
}

fn reference(torrent: &str, file: i32) -> String {
    format!("torbox:{torrent}:{file}")
}

fn remote(item: &Value) -> RemoteTorrent {
    RemoteTorrent {
        id: str_of(item, "id"),
        filename: str_of(item, "name"),
        bytes: num(item, "size").max(0) as u64,
        progress: (decimal(item, "progress") * 100.0).clamp(0.0, 100.0),
        status: state(item),
        added: date(item, "created_at"),
        speed: Some(num(item, "download_speed").max(0) as u64),
        seeders: Some(num(item, "seeds")),
    }
}

#[async_trait]
impl Debrid for TorBox {
    fn provider(&self) -> Provider {
        Provider::TorBox
    }

    async fn user(&self) -> Result<User> {
        let user = self.api.send(Method::GET, "user/me", &|r| r, true).await?;
        let mut identity = str_of(&user, "auth_id");
        if identity.is_empty() {
            identity = str_of(&user, "id");
        }
        if identity.is_empty() {
            return Err(Error::invalid("TorBox: missing account ID."));
        }
        let plan = if num(&user, "plan") == 0 { "Free" } else { "Premium" };
        Ok(User { username: identity, kind: format!("TorBox · {plan}"), premium: 1, allows_downloads: true })
    }

    async fn add_magnet(&self, magnet: &str) -> Result<String> {
        self.create(|| Form::new().text("magnet", magnet.to_string())).await
    }

    async fn add_torrent(&self, bytes: Vec<u8>) -> Result<String> {
        self.create(|| Form::new().part("file", Part::bytes(bytes.clone()).file_name("upload.torrent"))).await
    }

    async fn info(&self, id: &str) -> Result<TorrentInfo> {
        let data = self.api.send(Method::GET, &format!("torrents/mylist?bypass_cache=true&id={}", escape(id)), &|r| r, true).await?;
        let Some(item) = items(&data).into_iter().find(|item| str_of(item, "id") == id) else {
            return Err(Error::Api {
                provider: Provider::TorBox,
                status: 404,
                code: "ITEM_NOT_FOUND".into(),
                message: "TorBox: ITEM_NOT_FOUND · Torrent not found.".into(),
                error_code: None,
            });
        };
        let files: Vec<TorrentFile> = items(prop(item, "files"))
            .into_iter()
            .map(|file| TorrentFile {
                id: i32::try_from(num(file, "id")).unwrap_or(i32::MAX),
                path: format!("/{}", str_of(file, "name").trim_start_matches('/')),
                bytes: num(file, "size").max(0) as u64,
                selected: 1,
            })
            .collect();
        let mut unique: Vec<i32> = files.iter().map(|f| f.id).collect();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != files.len() {
            return Err(Error::invalid("TorBox: duplicate file IDs."));
        }
        let links: Vec<String> = files.iter().map(|file| reference(id, file.id)).collect();
        {
            let mut cache = self.files.lock().unwrap();
            for (link, file) in links.iter().zip(&files) {
                cache.insert(link.clone(), file.clone());
            }
        }
        Ok(TorrentInfo {
            id: id.to_string(),
            filename: str_of(item, "name"),
            status: state(item),
            progress: (decimal(item, "progress") * 100.0).clamp(0.0, 100.0),
            bytes: num(item, "size").max(0) as u64,
            files,
            links,
            seeders: Some(num(item, "seeds")),
            speed: Some(num(item, "download_speed").max(0) as u64),
        })
    }

    async fn list(&self) -> Result<Vec<RemoteTorrent>> {
        let mut all: Vec<RemoteTorrent> = Vec::new();
        let mut offset = 0;
        while offset < 100_000 {
            let data = self.api.send(Method::GET, &format!("torrents/mylist?bypass_cache=true&offset={offset}&limit=1000"), &|r| r, true).await?;
            let batch = items(&data);
            all.extend(batch.iter().map(|item| remote(item)));
            if batch.len() < 1000 {
                break;
            }
            offset += 1000;
        }
        let mut seen = std::collections::HashSet::new();
        all.retain(|torrent| seen.insert(torrent.id.clone()));
        Ok(all)
    }

    async fn select_all(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn select_files(&self, _id: &str, _file_ids: &[i32]) -> Result<()> {
        Ok(())
    }

    async fn unrestrict(&self, link: &str) -> Result<Unrestricted> {
        let parts: Vec<&str> = link.split(':').collect();
        if parts.len() != 3 || parts[0] != "torbox" {
            return Err(Error::invalid("TorBox: invalid file reference."));
        }
        let cached = self.files.lock().unwrap().get(link).cloned();
        let file = match cached {
            Some(file) => file,
            None => {
                self.info(parts[1]).await?;
                self.files.lock().unwrap().get(link).cloned().ok_or_else(|| Error::invalid("TorBox: missing file metadata."))?
            }
        };
        // TorBox requires the token as a query parameter for requestdl. It exists only in this request, never in queue
        // references, settings, status messages or persisted download links.
        let path = format!(
            "torrents/requestdl?token={}&torrent_id={}&file_id={}&zip_link=false&redirect=false",
            escape(&self.token),
            escape(parts[1]),
            escape(parts[2])
        );
        let data = self.api.send(Method::GET, &path, &|r| r, true).await?;
        let url = data.as_str().unwrap_or_default().to_string();
        match url::Url::parse(&url) {
            Ok(parsed) if parsed.scheme() == "https" => {}
            _ => return Err(Error::invalid("TorBox: invalid download URL.")),
        }
        let name = file.path.replace('\\', "/");
        Ok(Unrestricted { filename: name.rsplit('/').next().unwrap_or("").to_string(), filesize: file.bytes, download: url })
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let number: i64 = id.parse().map_err(|_| Error::invalid("TorBox: invalid torrent ID."))?;
        let body = json!({ "torrent_id": number, "operation": "delete", "all": false });
        self.api.send(Method::POST, "torrents/controltorrent", &|r| r.json(&body), false).await?;
        Ok(())
    }
}

#[allow(dead_code)]
fn _unused(_: Msg) {}
