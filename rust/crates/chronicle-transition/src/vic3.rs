//! What Chronicle can learn from a Victoria 3 save without decoding it.
//!
//! Verified on two real saves (Victoria 3 1.5.7 and 1.5.12): `SAV01…` header, then either a
//! metadata block followed by a ZIP, or a ZIP with `gamestate` + `meta` entries. Both saves
//! were token-encoded binary, which Chronicle cannot decode (token tables may not be
//! distributed). Plain strings inside the metadata are still readable: the game version
//! (e.g. "1.5.12") and the save's label (the player's country name). The in-game date is
//! taken from the file name when it has one (prussia_1885_04_13.v3).

use std::path::Path;

use chronicle_core::PartialDate;
use paradox_parser::container::{Encoding, archive, sniff_encoding};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Vic3SaveInfo {
    pub version: Option<String>,
    /// Best effort: the first readable string after the version in the metadata.
    pub label: Option<String>,
    pub binary: bool,
    pub date_from_name: Option<PartialDate>,
}

fn printable_strings(bytes: &[u8], min: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for &b in bytes {
        if (0x20..0x7f).contains(&b) {
            cur.push(b);
        } else {
            if cur.len() >= min {
                out.push(String::from_utf8_lossy(&cur).into_owned());
            }
            cur.clear();
        }
    }
    if cur.len() >= min {
        out.push(String::from_utf8_lossy(&cur).into_owned());
    }
    out
}

fn version_in(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let digits = |p: &str| -> String { p.chars().take_while(|c| c.is_ascii_digit()).collect() };
    let first: String = parts[0].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
    let (a, b, c) = (first, digits(parts[1]), digits(parts[2]));
    (!a.is_empty() && !b.is_empty() && !c.is_empty()).then(|| format!("{a}.{b}.{c}"))
}

/// "prussia_1885_04_13.v3" → 1885.4.13
pub fn date_from_file_name(path: &Path) -> Option<PartialDate> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let nums: Vec<&str> = stem.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).collect();
    nums.windows(3).find_map(|w| {
        let (y, m, d) = (w[0].parse::<i32>().ok()?, w[1].parse::<u8>().ok()?, w[2].parse::<u8>().ok()?);
        (w[0].len() == 4 && (1..=12).contains(&m) && (1..=31).contains(&d)).then_some(PartialDate::ymd(y, m, d))
    })
}

pub fn inspect(path: &Path) -> std::io::Result<Vic3SaveInfo> {
    let bytes = std::fs::read(path)?;
    let mut info = Vic3SaveInfo { date_from_name: date_from_file_name(path), ..Default::default() };
    let header_end = bytes.iter().position(|&b| b == b'\n').map(|i| i + 1).unwrap_or(0);
    let zip_at = bytes.windows(4).take(1 << 20).position(|w| w == b"PK\x03\x04");
    let meta: Vec<u8> = match zip_at {
        Some(at) if at > header_end => bytes[header_end..at].to_vec(),
        Some(at) => archive::read_entry(&bytes[at..], "meta").unwrap_or_default(),
        None => bytes[header_end..bytes.len().min(header_end + 64 * 1024)].to_vec(),
    };
    info.binary = sniff_encoding(&meta) == Encoding::Binary;
    let strings = printable_strings(&meta, 4);
    if let Some(i) = strings.iter().position(|s| version_in(s).is_some()) {
        info.version = version_in(&strings[i]);
        info.label = strings.get(i + 1).cloned().filter(|s| s.chars().any(|c| c.is_alphabetic()));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_strings_from_binary_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("prussia_1885_04_13.v3");
        let mut file = b"SAV0103da6d73a800000296\n".to_vec();
        // shape of the real metadata: token, type, length, string …
        file.extend_from_slice(b"\xadU\x01\x00\x03\x00\x8f\x05\x01\x00\x0c\x00\x01\x02\x03\x04\x01\x00\x0f\x00\x06\x001.5.12E2\x01\x00");
        file.extend_from_slice(b"\x0f\x00\x17\x00United States of Europe\x01\x00\x0f\x00\x0b\x00great_power");
        file.extend_from_slice(b"PK\x03\x04rest-of-zip");
        std::fs::write(&path, &file).unwrap();
        let info = inspect(&path).unwrap();
        assert!(info.binary);
        assert_eq!(info.version.as_deref(), Some("1.5.12"));
        assert_eq!(info.label.as_deref(), Some("United States of Europe"));
        assert_eq!(info.date_from_name, Some(PartialDate::ymd(1885, 4, 13)));
    }

    #[test]
    fn date_only_when_the_name_has_one() {
        assert_eq!(date_from_file_name(Path::new("autosave_exit.v3")), None);
        assert_eq!(date_from_file_name(Path::new("save_1836_1_1.v3")), Some(PartialDate::ymd(1836, 1, 1)));
    }
}
