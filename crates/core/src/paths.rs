//! Filesystem safety: sanitize torrent paths, keep them under the download root and refuse links/junctions.

use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3",
    "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn is_reserved(name: &str) -> bool {
    // CON, CON.txt, con.tar.gz are all reserved on Windows; sanitize on every OS so archives stay portable.
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    RESERVED.contains(&stem.as_str())
}

/// One safe file or directory name.
pub fn segment(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let mut clean = mapped.trim().trim_end_matches(['.', ' ']).to_string();
    if matches!(clean.as_str(), "" | "." | "..") {
        clean = "unnamed".into();
    }
    if is_reserved(&clean) {
        clean = format!("_{clean}");
    }
    if clean.chars().count() > 130 {
        let head: String = clean.chars().take(100).collect();
        let hash = hex_upper(&Sha256::digest(clean.as_bytes()));
        let extension = Path::new(&clean).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let extension: String = extension.chars().take(12).collect();
        return format!("{head}_{}{extension}", &hash[..12]);
    }
    clean
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// A torrent-relative path as `/`-separated sanitized segments (stored in queues, joined per OS when used).
pub fn relative(path: &str) -> Result<String> {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|part| !part.is_empty()).collect();
    if parts.iter().any(|part| *part == "." || *part == "..") {
        return Err(Error::Operation(crate::models::Msg::plain("Torrent có đường dẫn không an toàn.")));
    }
    if parts.is_empty() {
        return Ok("unnamed".into());
    }
    Ok(parts.iter().map(|part| segment(part)).collect::<Vec<_>>().join("/"))
}

fn same(a: &Path, b: &Path) -> bool {
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    if cfg!(windows) { a.eq_ignore_ascii_case(&b) } else { a == b }
}

fn starts_with(path: &Path, root: &Path) -> bool {
    let mut path_parts = path.components();
    for root_part in root.components() {
        match path_parts.next() {
            Some(part) if same(Path::new(part.as_os_str()), Path::new(root_part.as_os_str())) => {}
            _ => return false,
        }
    }
    true
}

/// Lexically resolve `relative` below `root`. Never touches the filesystem (the file may not exist yet) and never
/// accepts `..`, drive letters or absolute components.
pub fn under_root(root: &Path, relative: &str) -> Result<PathBuf> {
    let root = std::path::absolute(root).map_err(Error::Io)?;
    let mut full = root.clone();
    for part in relative.replace('\\', "/").split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(Error::Operation(crate::models::Msg::plain("Đường dẫn nằm ngoài thư mục tải."))),
            other => {
                // A component like `C:` or a UNC prefix would reset the base when pushed.
                if matches!(Path::new(other).components().next(), Some(Component::Prefix(_) | Component::RootDir)) {
                    return Err(Error::Operation(crate::models::Msg::plain("Đường dẫn nằm ngoài thư mục tải.")));
                }
                full.push(other);
            }
        }
    }
    if !starts_with(&full, &root) {
        return Err(Error::Operation(crate::models::Msg::plain("Đường dẫn nằm ngoài thư mục tải.")));
    }
    Ok(full)
}

fn is_link(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return false };
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT covers junctions and mount points as well as symlinks.
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

/// Refuse a destination whose parents (up to `root`) or whose own entry is a symlink/junction.
pub fn ensure_no_reparse_points(root: &Path, destination: &Path) -> Result<()> {
    let root = std::path::absolute(root).map_err(Error::Io)?;
    let mut current = destination.parent().map(Path::to_path_buf);
    while let Some(dir) = current {
        if dir.exists() && is_link(&dir) {
            return Err(Error::Operation(crate::models::Msg::plain(
                "Thư mục tải có junction/symlink. Hãy chọn một thư mục thông thường.",
            )));
        }
        if same(&dir, &root) {
            break;
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    if destination.exists() && is_link(destination) {
        return Err(Error::Operation(crate::models::Msg::plain("File đích là symlink.")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_sanitized() {
        assert_eq!(segment("a<b>c:d"), "a_b_c_d");
        assert_eq!(segment("  name. . "), "name");
        assert_eq!(segment(".."), "unnamed");
        assert_eq!(segment(""), "unnamed");
        assert_eq!(segment("CON"), "_CON");
        assert_eq!(segment("nul.txt"), "_nul.txt");
        assert_eq!(segment("COM1.tar.gz"), "_COM1.tar.gz");
        assert_eq!(segment("console.txt"), "console.txt");
    }

    #[test]
    fn long_names_are_shortened_with_a_stable_hash() {
        let name = format!("{}.mkv", "x".repeat(200));
        let short = segment(&name);
        assert!(short.chars().count() <= 100 + 1 + 12 + 12);
        assert!(short.ends_with(".mkv"));
        assert_eq!(short, segment(&name));
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(relative("../evil.mkv").is_err());
        assert!(relative("a/../../b").is_err());
        assert_eq!(relative("/Show/S01/ep.mkv").unwrap(), "Show/S01/ep.mkv");
        assert_eq!(relative("\\Show\\ep.mkv").unwrap(), "Show/ep.mkv");
        let root = tempfile::tempdir().unwrap();
        assert!(under_root(root.path(), "../x").is_err());
        assert!(under_root(root.path(), "a/../../x").is_err());
        assert!(under_root(root.path(), "C:/Windows/x").is_err() || cfg!(not(windows)));
        assert!(under_root(root.path(), "a/b.mkv").unwrap().starts_with(std::path::absolute(root.path()).unwrap()));
    }

    #[test]
    fn case_only_matters_on_windows() {
        let root = tempfile::tempdir().unwrap();
        // `Sub` and `sub` are different directories on Linux; the root prefix check must stay exact there.
        let inside = under_root(root.path(), "Sub/file.bin").unwrap();
        assert!(starts_with(&inside, &std::path::absolute(root.path()).unwrap()));
        let upper = PathBuf::from(root.path().to_string_lossy().to_uppercase());
        assert_eq!(starts_with(&inside, &upper), cfg!(windows));
    }

    #[test]
    fn symlinked_parents_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let link = root.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_dir(outside.path(), &link).is_err() {
            return; // creating symlinks needs developer mode or elevation on Windows
        }
        let destination = link.join("file.bin");
        assert!(ensure_no_reparse_points(root.path(), &destination).is_err());
        assert!(ensure_no_reparse_points(root.path(), &root.path().join("plain/file.bin")).is_ok());
    }
}
