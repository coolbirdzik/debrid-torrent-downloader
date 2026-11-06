//! Turn pasted text into torrent sources: magnets (hex or base32), .torrent URLs and existing .torrent files.

use crate::models::Msg;
use percent_encoding::percent_decode_str;
use std::collections::HashSet;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSource {
    pub source: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct ParseResult {
    pub sources: Vec<ParsedSource>,
    pub errors: Vec<Msg>,
}

pub fn parse(text: &str) -> ParseResult {
    let mut result = ParseResult::default();
    let mut keys: HashSet<String> = HashSet::new();
    for line in text.split(['\r', '\n']).map(str::trim).filter(|line| !line.is_empty()) {
        // Several magnets pasted on one line are accepted; local paths may contain spaces.
        let tokens: Vec<String> = if line.len() >= 7 && line[..7].eq_ignore_ascii_case("magnet:") {
            split_magnets(line)
        } else {
            vec![line.to_string()]
        };
        for token in tokens {
            let source = token.trim().trim_matches('"').to_string();
            let parsed = parse_one(&source);
            match parsed {
                None => result.errors.push(Msg::new(
                    "Dòng {0}: cần magnet, URL .torrent hoặc đường dẫn file .torrent tồn tại.",
                    &[&(result.sources.len() + result.errors.len() + 1)],
                )),
                Some(parsed) => {
                    if keys.insert(parsed.key.to_lowercase()) {
                        result.sources.push(parsed);
                    }
                }
            }
        }
    }
    result
}

fn split_magnets(line: &str) -> Vec<String> {
    let lower = line.to_ascii_lowercase();
    let mut cuts = vec![0];
    for (index, _) in lower.match_indices("magnet:?") {
        if index > 0 && line.as_bytes()[index - 1].is_ascii_whitespace() {
            cuts.push(index);
        }
    }
    cuts.push(line.len());
    cuts.windows(2).map(|w| line[w[0]..w[1]].trim().to_string()).filter(|part| !part.is_empty()).collect()
}

fn parse_one(source: &str) -> Option<ParsedSource> {
    if source.len() >= 8 && source[..8].eq_ignore_ascii_case("magnet:?") {
        return parse_magnet(source);
    }
    if let Ok(url) = url::Url::parse(source) {
        if matches!(url.scheme(), "http" | "https") {
            let name = url
                .path_segments()
                .and_then(|segments| segments.last().map(|s| percent_decode_str(s).decode_utf8_lossy().to_string()))
                .filter(|name| !name.is_empty())
                .or_else(|| url.host_str().map(str::to_string))
                .unwrap_or_default();
            return Some(ParsedSource { source: source.to_string(), key: url.to_string(), name });
        }
    }
    let path = Path::new(source);
    let is_torrent = source.to_ascii_lowercase().ends_with(".torrent");
    if path.is_absolute() && is_torrent && path.is_file() {
        let full = std::path::absolute(path).ok()?.to_string_lossy().to_string();
        let name = path.file_stem()?.to_string_lossy().to_string();
        return Some(ParsedSource { source: full.clone(), key: full, name });
    }
    None
}

fn parse_magnet(source: &str) -> Option<ParsedSource> {
    let query: Vec<(String, String)> = source[8..]
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_string(), percent_decode_str(value).decode_utf8_lossy().to_string()))
        .collect();
    let value = query
        .iter()
        .find(|(key, value)| key.eq_ignore_ascii_case("xt") && value.len() >= 9 && value[..9].eq_ignore_ascii_case("urn:btih:"))
        .map(|(_, value)| value.clone())?;
    let hash = &value[9..];
    if !is_hash(hash) {
        return None;
    }
    let name = query.iter().find(|(key, _)| key.eq_ignore_ascii_case("dn")).map(|(_, value)| value.clone()).unwrap_or_default();
    let name = if name.trim().is_empty() {
        format!("Magnet · {}", hash.chars().take(12).collect::<String>())
    } else {
        name.replace('+', " ")
    };
    Some(ParsedSource { source: source.to_string(), key: format!("btih:{}", normalize_hash(hash)), name })
}

fn is_hash(value: &str) -> bool {
    (value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit()))
        || (value.len() == 32 && value.chars().all(|c| c.is_ascii_alphabetic() || ('2'..='7').contains(&c)))
}

fn normalize_hash(value: &str) -> String {
    if value.len() == 40 {
        return value.to_ascii_uppercase();
    }
    const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bytes = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in value.to_ascii_uppercase().chars() {
        buffer = (buffer << 5) | ALPHABET.find(c).unwrap_or(0) as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn hex_and_base32_magnets_are_deduplicated() {
        // base32 of the same 20 bytes
        let base32 = "AEjwKz2m3Re6Tobe2y3JUXECEG5HG7UrVQ"; // not valid length on purpose below
        let _ = base32;
        let bytes: Vec<u8> = (0..20).map(|i| (i * 13 + 7) as u8).collect();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let (mut bits, mut buffer, mut b32) = (0u32, 0u32, String::new());
        for byte in &bytes {
            buffer = (buffer << 8) | *byte as u32;
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                b32.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
            }
            buffer &= (1 << bits) - 1;
        }
        assert_eq!(b32.len(), 32);
        let text = format!("magnet:?xt=urn:btih:{hex}&dn=First+Show\nmagnet:?xt=urn:btih:{b32}\n");
        let result = parse(&text);
        assert_eq!(result.sources.len(), 1, "hex and base32 forms of one hash are one source");
        assert_eq!(result.sources[0].name, "First Show");
        assert_eq!(result.sources[0].key, format!("btih:{}", hex.to_uppercase()));
    }

    #[test]
    fn several_magnets_on_one_line_and_invalid_lines() {
        let other = "f".repeat(40);
        let text = format!("magnet:?xt=urn:btih:{HEX} magnet:?xt=urn:btih:{other}&dn=Two\nnot a source\n\"https://example.com/files/show.torrent\"");
        let result = parse(&text);
        assert_eq!(result.sources.len(), 3);
        assert_eq!(result.sources[1].name, "Two");
        assert_eq!(result.sources[2].name, "show.torrent");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].args, vec!["3".to_string()]);
    }

    #[test]
    fn rejects_bad_hashes_and_missing_files() {
        let result = parse("magnet:?xt=urn:btih:xyz\nC:\\nope\\missing.torrent\n/nope/missing.torrent");
        assert!(result.sources.is_empty());
        assert_eq!(result.errors.len(), 3);
    }

    #[test]
    fn existing_torrent_files_are_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("My Show.torrent");
        std::fs::write(&file, b"d4:infod4:name1:aee").unwrap();
        let result = parse(&file.to_string_lossy());
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.sources[0].name, "My Show");
    }
}
