//! Streaming: which files of a finished cloud torrent can be played, and how each maps to a service link.

use crate::models::TorrentInfo;
use serde::Serialize;

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mkv", "webm", "mov", "avi", "ts", "m2ts", "wmv", "flv", "mpg", "mpeg", "ogv"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFile {
    /// Index into `TorrentInfo::links`; this is what `stream_url` resolves.
    pub link_index: usize,
    /// The service's file ID, as stored in a Library draft.
    pub file_id: i32,
    pub path: String,
    pub bytes: u64,
}

/// What the player needs to start: the playable files once the service has the torrent, or how far along it is.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamList {
    pub ready: bool,
    pub status: String,
    pub progress: f64,
    pub files: Vec<StreamFile>,
}

pub fn failed_status(status: &str) -> bool {
    matches!(status, "magnet_error" | "error" | "virus" | "dead")
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSource {
    pub url: String,
    pub filename: String,
    pub bytes: u64,
}

pub fn is_video(path: &str) -> bool {
    path.rsplit('.').next().is_some_and(|ext| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(ext)))
}

pub fn basename(path: &str) -> String {
    path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_string()
}

/// Selected files map to links one-to-one, in order, only when the counts match. Real-Debrid can return a different
/// number of links (split archives); then no file can be tied to a link safely and nothing is offered.
pub fn playable_files(info: &TorrentInfo) -> Vec<StreamFile> {
    let selected: Vec<_> = info.files.iter().filter(|f| f.selected == 1).collect();
    if selected.len() != info.links.len() {
        return Vec::new();
    }
    selected
        .into_iter()
        .enumerate()
        .filter(|(_, f)| is_video(&f.path))
        .map(|(index, f)| StreamFile { link_index: index, file_id: f.id, path: f.path.trim_start_matches('/').to_string(), bytes: f.bytes })
        .collect()
}

/// The file a link is expected to resolve to, used to refuse a link that changed since the list was shown.
pub fn expected_at(info: &TorrentInfo, link_index: usize) -> Option<(String, u64)> {
    let selected: Vec<_> = info.files.iter().filter(|f| f.selected == 1).collect();
    if selected.len() != info.links.len() {
        return None;
    }
    selected.get(link_index).map(|f| (basename(&f.path), f.bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TorrentFile;

    fn file(id: i32, path: &str, selected: i32) -> TorrentFile {
        TorrentFile { id, path: path.into(), bytes: 10, selected }
    }

    #[test]
    fn detects_video_extensions_case_insensitively() {
        assert!(is_video("/a/Movie.MKV"));
        assert!(is_video("clip.mp4"));
        assert!(!is_video("readme.txt"));
        assert!(!is_video("noextension"));
    }

    #[test]
    fn maps_only_selected_files_to_links_in_order() {
        let info = TorrentInfo {
            files: vec![file(1, "/x/a.nfo", 0), file(2, "/x/b.mkv", 1), file(3, "/x/c.txt", 1), file(4, "/x/d.mp4", 1)],
            links: vec!["l0".into(), "l1".into(), "l2".into()],
            ..Default::default()
        };
        let playable = playable_files(&info);
        assert_eq!(playable.iter().map(|f| (f.link_index, f.path.as_str())).collect::<Vec<_>>(), vec![(0, "x/b.mkv"), (2, "x/d.mp4")]);
        assert_eq!(expected_at(&info, 2), Some(("d.mp4".to_string(), 10)));
    }

    #[test]
    fn offers_nothing_when_link_count_differs() {
        let info = TorrentInfo { files: vec![file(1, "/a.mkv", 1), file(2, "/b.mkv", 1)], links: vec!["only-one".into()], ..Default::default() };
        assert!(playable_files(&info).is_empty());
        assert_eq!(expected_at(&info, 0), None);
    }
}
