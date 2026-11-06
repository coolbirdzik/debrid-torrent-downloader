//! Filtering a torrent's files by folder, search terms and "videos only".

use crate::models::TorrentFile;

const VIDEO: [&str; 18] = [
    "mp4", "mkv", "avi", "mov", "wmv", "webm", "m4v", "ts", "m2ts", "mts", "mpg", "mpeg", "flv", "vob", "ogv", "3gp", "divx", "rmvb",
];

pub fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn extension(path: &str) -> String {
    let name = normalize(path);
    let name = name.rsplit('/').next().unwrap_or("");
    match name.rfind('.') {
        Some(index) if index + 1 < name.len() => name[index + 1..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

pub fn is_video(path: &str) -> bool {
    VIDEO.contains(&extension(path).as_str())
}

pub fn filter<'a>(files: impl IntoIterator<Item = &'a TorrentFile>, query: &str, videos_only: bool, folder: &str) -> Vec<&'a TorrentFile> {
    let folder = normalize(folder);
    let directory = folder.trim_matches('/');
    let prefix = if directory.is_empty() { String::new() } else { format!("{}/", directory.to_lowercase()) };
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    files
        .into_iter()
        .filter(|file| {
            let path = normalize(&file.path).to_lowercase();
            path.trim_start_matches('/').starts_with(&prefix) && (!videos_only || is_video(&file.path)) && terms.iter().all(|term| path.contains(term))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: i32, path: &str) -> TorrentFile {
        TorrentFile { id, path: path.into(), bytes: 1, selected: 1 }
    }

    #[test]
    fn videos_are_found_through_nested_folders_with_exact_boundaries() {
        let files = vec![
            file(1, "/Show/S01/ep1.MKV"),
            file(2, "/Show/S01/notes.txt"),
            file(3, "/Show/S010/ep9.mp4"),
            file(4, "/Show 2/S01/ep1.mkv"),
            file(5, "/Show/Extras/Trailer.mov"),
        ];
        let ids = |found: Vec<&TorrentFile>| found.iter().map(|f| f.id).collect::<Vec<_>>();
        assert_eq!(ids(filter(&files, "", true, "Show/S01")), vec![1]);
        assert_eq!(ids(filter(&files, "", true, "/Show/")), vec![1, 3, 5]);
        assert_eq!(ids(filter(&files, "s01 ep1", false, "")), vec![1, 4]);
        assert_eq!(ids(filter(&files, "TRAILER", false, "Show")), vec![5]);
    }

    #[test]
    fn extension_detection() {
        assert!(is_video("a/b.MP4"));
        assert!(!is_video("a/b.mp4.txt"));
        assert!(!is_video("a/b"));
        assert_eq!(extension("/x/.hidden"), "hidden");
    }
}
