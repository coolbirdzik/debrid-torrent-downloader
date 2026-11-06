//! Update check against the latest published GitHub release.
//!
//! This only *finds* a newer version and the right installer for this platform; the UI opens it in the browser.
//! (An in-app installer would need signed updater artifacts, which the release pipeline does not produce.)

use rdbatch_core::models::Msg;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::Duration;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/coolbirdzik/debrid-torrent-downloader/releases/latest";
/// Only release pages and assets of this owner may be opened from the UI.
const TRUSTED_PREFIX: &str = "https://github.com/coolbirdzik/";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    /// `None` when the repository has no published release yet.
    pub latest: Option<String>,
    pub available: bool,
    pub page_url: Option<String>,
    pub download_url: Option<String>,
    pub notes: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// True for URLs the app is willing to open: https GitHub release pages and release assets of this project's owner.
pub fn is_trusted_url(url: &str) -> bool {
    url.starts_with(TRUSTED_PREFIX) && url.contains("/releases/") && !url.contains("..")
}

pub async fn check(current: &str) -> Result<UpdateInfo, Msg> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("DebridDock")
        .build()
        .map_err(|e| Msg::new("Không kiểm tra được bản cập nhật: {0}", &[&e]))?;
    let response = client
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| Msg::plain("Không kết nối được GitHub. Kiểm tra mạng rồi thử lại."))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(none_published(current));
    }
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(Msg::plain("GitHub đang giới hạn số lần kiểm tra. Hãy thử lại sau ít phút."));
    }
    if !status.is_success() {
        return Err(Msg::new("GitHub trả về lỗi {0}.", &[&status.as_u16()]));
    }
    let body = response.text().await.map_err(|_| Msg::plain("Không kết nối được GitHub. Kiểm tra mạng rồi thử lại."))?;
    evaluate(&body, current, std::env::consts::OS, std::env::consts::ARCH)
}

fn none_published(current: &str) -> UpdateInfo {
    UpdateInfo { current: current.to_string(), latest: None, available: false, page_url: None, download_url: None, notes: String::new() }
}

fn evaluate(json: &str, current: &str, os: &str, arch: &str) -> Result<UpdateInfo, Msg> {
    let release: Release = serde_json::from_str(json).map_err(|_| Msg::plain("GitHub trả về dữ liệu không đọc được."))?;
    if release.draft || release.prerelease {
        return Ok(none_published(current));
    }
    let latest = release.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    let available = match (Version::parse(&latest), Version::parse(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    };
    let page_url = Some(release.html_url.clone()).filter(|url| is_trusted_url(url));
    let download_url = pick_asset(&release.assets, os, arch).filter(|url| is_trusted_url(url)).or_else(|| page_url.clone());
    Ok(UpdateInfo {
        current: current.to_string(),
        latest: Some(latest),
        available,
        page_url,
        download_url,
        notes: release.body.unwrap_or_default().trim().to_string(),
    })
}

/// The installer for this platform, best format first. Checksum files and other platforms' packages never match.
fn pick_asset(assets: &[Asset], os: &str, arch: &str) -> Option<String> {
    let formats: &[&str] = match os {
        "windows" => &["-setup.exe", ".msi"],
        "macos" => &[".dmg"],
        "linux" => &[".appimage", ".deb", ".rpm"],
        _ => return None,
    };
    let (mine, others): (&[&str], &[&str]) = match arch {
        "aarch64" => (&["aarch64", "arm64"], &["x86_64", "amd64", "x64"]),
        _ => (&["x86_64", "amd64", "x64"], &["aarch64", "arm64"]),
    };
    for format in formats {
        let matching: Vec<&Asset> = assets.iter().filter(|a| a.name.to_lowercase().ends_with(format)).collect();
        let lower = |a: &&Asset| a.name.to_lowercase();
        // Prefer this CPU's build; a universal or unlabeled file is fine; never another CPU's.
        let best = matching
            .iter()
            .find(|a| mine.iter().any(|m| lower(a).contains(m)))
            .or_else(|| matching.iter().find(|a| !others.iter().any(|o| lower(a).contains(o))));
        if let Some(asset) = best {
            return Some(asset.browser_download_url.clone());
        }
    }
    None
}

/// `MAJOR.MINOR.PATCH[-prerelease][+build]`, with an optional `v` prefix. Implemented here to avoid a dependency.
#[derive(Debug, PartialEq, Eq)]
struct Version {
    core: [u64; 3],
    pre: Vec<String>,
}

impl Version {
    fn parse(text: &str) -> Option<Version> {
        let text = text.trim().trim_start_matches(['v', 'V']);
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, pre.split('.').map(str::to_string).collect()),
            None => (text, Vec::new()),
        };
        let mut numbers = core.split('.');
        let mut part = || numbers.next().and_then(|n| n.parse::<u64>().ok());
        let parsed = [part()?, part()?, part()?];
        if numbers.next().is_some() {
            return None;
        }
        Some(Version { core: parsed, pre })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            // A release is newer than any of its pre-releases.
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => compare_identifiers(&self.pre, &other.pre),
        })
    }
}

fn compare_identifiers(a: &[String], b: &[String]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|| panic!("{text} should parse"))
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(v("0.10.0") > v("0.9.0"));
        assert!(v("v1.0.0") > v("0.99.99"));
        assert!(v("1.2.3") == v("V1.2.3+build.5"));
        assert!(v("1.0.0") > v("1.0.0-beta"));
        assert!(v("1.0.0-beta.2") > v("1.0.0-beta.1"));
        assert!(v("1.0.0-beta.11") > v("1.0.0-beta.2"));
        assert!(v("1.0.0-rc.1") > v("1.0.0-beta.9"));
        assert!(Version::parse("1.0").is_none());
        assert!(Version::parse("1.0.0.0").is_none());
        assert!(Version::parse("latest").is_none());
    }

    const RELEASE: &str = r#"{
        "tag_name": "v0.4.0", "html_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/tag/v0.4.0",
        "draft": false, "prerelease": false, "body": "  - Faster imports\n",
        "assets": [
            {"name": "DebridDock_0.4.0_x64-setup.exe", "browser_download_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/DebridDock_0.4.0_x64-setup.exe"},
            {"name": "DebridDock_0.4.0_x64_en-US.msi", "browser_download_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/DebridDock_0.4.0_x64_en-US.msi"},
            {"name": "DebridDock_0.4.0_universal.dmg", "browser_download_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/DebridDock_0.4.0_universal.dmg"},
            {"name": "DebridDock_0.4.0_amd64.deb", "browser_download_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/DebridDock_0.4.0_amd64.deb"},
            {"name": "DebridDock_0.4.0_amd64.AppImage", "browser_download_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/DebridDock_0.4.0_amd64.AppImage"},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/SHA256SUMS.txt"}
        ]
    }"#;

    fn file(url: &Option<String>) -> &str {
        url.as_deref().unwrap().rsplit('/').next().unwrap()
    }

    #[test]
    fn newer_release_picks_the_platform_installer() {
        let windows = evaluate(RELEASE, "0.3.0", "windows", "x86_64").unwrap();
        assert!(windows.available);
        assert_eq!(windows.latest.as_deref(), Some("0.4.0"));
        assert_eq!(file(&windows.download_url), "DebridDock_0.4.0_x64-setup.exe");
        assert_eq!(windows.notes, "- Faster imports");
        assert_eq!(file(&evaluate(RELEASE, "0.3.0", "macos", "aarch64").unwrap().download_url), "DebridDock_0.4.0_universal.dmg");
        assert_eq!(file(&evaluate(RELEASE, "0.3.0", "linux", "x86_64").unwrap().download_url), "DebridDock_0.4.0_amd64.AppImage");
    }

    #[test]
    fn falls_back_to_the_release_page_without_a_matching_asset() {
        // aarch64 Linux has no arm64 package in this release, and an amd64 one must not be offered.
        let info = evaluate(RELEASE, "0.3.0", "linux", "aarch64").unwrap();
        assert_eq!(info.download_url, info.page_url);
        assert!(info.page_url.unwrap().ends_with("/releases/tag/v0.4.0"));
    }

    #[test]
    fn same_or_older_release_is_not_an_update() {
        assert!(!evaluate(RELEASE, "0.4.0", "windows", "x86_64").unwrap().available);
        assert!(!evaluate(RELEASE, "0.5.0", "windows", "x86_64").unwrap().available);
        assert!(evaluate(RELEASE, "0.4.0-beta.1", "windows", "x86_64").unwrap().available);
    }

    #[test]
    fn drafts_prereleases_and_foreign_links_are_ignored() {
        let draft = RELEASE.replace("\"draft\": false", "\"draft\": true");
        assert_eq!(evaluate(&draft, "0.3.0", "windows", "x86_64").unwrap().latest, None);
        let pre = RELEASE.replace("\"prerelease\": false", "\"prerelease\": true");
        assert!(!evaluate(&pre, "0.3.0", "windows", "x86_64").unwrap().available);
        let foreign = RELEASE.replace("https://github.com/coolbirdzik/", "https://evil.example/");
        let info = evaluate(&foreign, "0.3.0", "windows", "x86_64").unwrap();
        assert!(info.available);
        assert_eq!((info.page_url, info.download_url), (None, None));
        assert!(evaluate("{}", "0.3.0", "windows", "x86_64").is_err());
    }

    #[test]
    fn only_project_release_urls_are_trusted() {
        assert!(is_trusted_url("https://github.com/coolbirdzik/debrid-torrent-downloader/releases/tag/v1.0.0"));
        assert!(!is_trusted_url("http://github.com/coolbirdzik/x/releases/tag/v1"));
        assert!(!is_trusted_url("https://github.com/coolbirdzik/x/issues/1"));
        assert!(!is_trusted_url("https://github.com/someone-else/x/releases/tag/v1"));
        assert!(!is_trusted_url("file:///C:/Windows/System32/calc.exe"));
    }
}
