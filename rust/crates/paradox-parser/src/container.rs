//! Save container detection. Only *packaging* is detected here (header line, archive type,
//! text vs binary). Game-internal fields are never interpreted in this module.
//!
//! ⚠️ Facts to confirm per game/version with real fixtures (tests/fixtures):
//! - several Clausewitz games start saves with a one-line header beginning with `SAV`;
//! - compressed saves are ZIP archives, possibly preceded by a header and/or plain-text
//!   metadata — so we search for the ZIP signature instead of assuming offset 0.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Plain,
    Zip,
    Gzip,
    Zstd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Windows1252,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Text(TextEncoding),
    /// Token-encoded binary Clausewitz. Requires a TokenResolver (not in MVP).
    Binary,
    /// Compressed payload; encoding is known only after decompression.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedSave {
    /// First line if the file starts with a `SAV` header (raw, uninterpreted).
    pub header_line: Option<String>,
    /// Offset where the payload after the header starts.
    pub payload_offset: usize,
    pub container: ContainerKind,
    /// For Zip: absolute offset of the ZIP signature; bytes in
    /// `payload_offset..archive_offset` are a plain prefix (often metadata).
    pub archive_offset: Option<usize>,
    pub encoding: Encoding,
}

const ZIP_MAGIC: &[u8] = b"PK\x03\x04";
const GZIP_MAGIC: &[u8] = b"\x1f\x8b";
const ZSTD_MAGIC: &[u8] = b"\x28\xb5\x2f\xfd";
const SAMPLE: usize = 64 * 1024;

/// Hearts of Iron IV saves start with a 7-byte magic instead of a `SAV` line.
/// `HOI4bin` verified on 1.19.3 (token-encoded binary); `HOI4txt` is the plain-text variant.
const HOI4_BIN: &[u8] = b"HOI4bin";
const HOI4_TXT: &[u8] = b"HOI4txt";
/// A ZIP signature is only trusted this close to the start, and only after a text prefix:
/// large binary payloads can contain the 4 signature bytes by chance.
const ZIP_SEARCH_WINDOW: usize = 16 * 1024 * 1024;

pub fn detect(bytes: &[u8]) -> DetectedSave {
    if bytes.starts_with(HOI4_BIN) || bytes.starts_with(HOI4_TXT) {
        let binary = bytes.starts_with(HOI4_BIN);
        return DetectedSave {
            header_line: Some(String::from_utf8_lossy(&bytes[..7]).into_owned()),
            payload_offset: 7,
            container: ContainerKind::Plain,
            archive_offset: None,
            encoding: if binary { Encoding::Binary } else { sniff_encoding(&bytes[7..]) },
        };
    }

    let (header_line, payload_offset) = read_header(bytes);
    let payload = &bytes[payload_offset..];

    let (container, archive_offset) = if payload.starts_with(GZIP_MAGIC) {
        (ContainerKind::Gzip, Some(payload_offset))
    } else if payload.starts_with(ZSTD_MAGIC) {
        (ContainerKind::Zstd, Some(payload_offset))
    } else if let Some(pos) = find(&payload[..payload.len().min(ZIP_SEARCH_WINDOW)], ZIP_MAGIC)
        .filter(|&pos| pos == 0 || !matches!(sniff_encoding(&payload[..pos]), Encoding::Binary))
    {
        (ContainerKind::Zip, Some(payload_offset + pos))
    } else {
        (ContainerKind::Plain, None)
    };

    let encoding = match container {
        ContainerKind::Plain => sniff_encoding(payload),
        _ => Encoding::Unknown,
    };

    DetectedSave { header_line, payload_offset, container, archive_offset, encoding }
}

/// Best-effort game version from the first bytes, e.g. "1.19.3" from
/// `Operation Postern v1.19.3.0.c01a` or "3.14.15" from `Circinus v3.14.15`.
/// Works on binary saves too because the version is stored as a plain string.
pub fn sniff_version(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(8192)];
    let mut i = 0;
    while i + 1 < head.len() {
        if head[i] == b'v' && head[i + 1].is_ascii_digit() && (i == 0 || !head[i - 1].is_ascii_alphanumeric()) {
            let mut parts: Vec<String> = Vec::new();
            let mut j = i + 1;
            loop {
                let start = j;
                while j < head.len() && head[j].is_ascii_digit() {
                    j += 1;
                }
                if j == start {
                    break;
                }
                parts.push(String::from_utf8_lossy(&head[start..j]).into_owned());
                if parts.len() == 3 || j >= head.len() || head[j] != b'.' {
                    break;
                }
                j += 1;
            }
            if parts.len() >= 2 {
                return Some(parts.join("."));
            }
        }
        i += 1;
    }
    None
}

fn read_header(bytes: &[u8]) -> (Option<String>, usize) {
    if !bytes.starts_with(b"SAV") {
        return (None, 0);
    }
    match bytes.iter().take(512).position(|&b| b == b'\n') {
        Some(nl) => {
            let line = String::from_utf8_lossy(&bytes[..nl]).trim_end_matches('\r').to_owned();
            (Some(line), nl + 1)
        }
        None => (None, 0),
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Heuristic: many control bytes ⇒ binary; valid UTF-8 ⇒ Utf8; else Windows-1252.
pub fn sniff_encoding(payload: &[u8]) -> Encoding {
    let sample = &payload[..payload.len().min(SAMPLE)];
    if sample.is_empty() {
        return Encoding::Text(TextEncoding::Utf8);
    }
    let control = sample
        .iter()
        .filter(|&&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r'))
        .count();
    if control * 100 > sample.len() {
        return Encoding::Binary;
    }
    match std::str::from_utf8(sample) {
        Ok(_) => Encoding::Text(TextEncoding::Utf8),
        // error_len() == None ⇒ sample cut in the middle of a valid multi-byte char
        Err(e) if e.error_len().is_none() => Encoding::Text(TextEncoding::Utf8),
        Err(_) => Encoding::Text(TextEncoding::Windows1252),
    }
}

#[cfg(feature = "zip")]
pub mod archive {
    use crate::error::{ParseError, Result};
    use std::io::{Cursor, Read};

    fn open(bytes: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>> {
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| ParseError::UnsupportedContainer(e.to_string()))
    }

    /// (name, uncompressed size) of every entry. `bytes` must start at the archive offset.
    pub fn entries(bytes: &[u8]) -> Result<Vec<(String, u64)>> {
        let mut a = open(bytes)?;
        (0..a.len())
            .map(|i| {
                let f = a.by_index(i).map_err(|e| ParseError::UnsupportedContainer(e.to_string()))?;
                Ok((f.name().to_owned(), f.size()))
            })
            .collect()
    }

    pub fn read_entry(bytes: &[u8], name: &str) -> Result<Vec<u8>> {
        let mut a = open(bytes)?;
        let mut f = a.by_name(name).map_err(|e| ParseError::UnsupportedContainer(e.to_string()))?;
        let mut out = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut out)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_without_header() {
        let d = detect(b"date=867.1.1\n");
        assert_eq!(d.header_line, None);
        assert_eq!(d.container, ContainerKind::Plain);
        assert_eq!(d.encoding, Encoding::Text(TextEncoding::Utf8));
    }

    #[test]
    fn header_then_text() {
        let d = detect(b"SAV0000test\r\nmeta = { }\n");
        assert_eq!(d.header_line.as_deref(), Some("SAV0000test"));
        assert_eq!(d.payload_offset, 13);
        assert_eq!(d.container, ContainerKind::Plain);
    }

    #[test]
    fn header_then_prefix_then_zip() {
        let mut v = b"SAVxyz\nmeta = { }\n".to_vec();
        let zip_at = v.len();
        v.extend_from_slice(b"PK\x03\x04rest");
        let d = detect(&v);
        assert_eq!(d.container, ContainerKind::Zip);
        assert_eq!(d.archive_offset, Some(zip_at));
    }

    #[test]
    fn hoi4_binary_header() {
        // Same layout as a real 1.19.3 save: magic, then token/type/len/value records.
        let mut v = b"HOI4bin".to_vec();
        v.extend_from_slice(b"\x35\x2a\x01\x00\x0f\x00\x03\x00GER");
        v.extend_from_slice(b"\x01\x00\x0f\x00\x27\x00Operation Postern v1.19.3.0.c01a (5632)");
        v.extend_from_slice(&[0u8, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 6]);
        let d = detect(&v);
        assert_eq!(d.header_line.as_deref(), Some("HOI4bin"));
        assert_eq!(d.payload_offset, 7);
        assert_eq!(d.container, ContainerKind::Plain);
        assert_eq!(d.encoding, Encoding::Binary);
        assert_eq!(sniff_version(&v).as_deref(), Some("1.19.3"));
    }

    #[test]
    fn hoi4_text_header_and_stellaris_version() {
        let d = detect(b"HOI4txt\nplayer=\"GER\"\n");
        assert_eq!(d.encoding, Encoding::Text(TextEncoding::Utf8));
        assert_eq!(sniff_version(b"version=\"Circinus v3.14.15\"").as_deref(), Some("3.14.15"));
        assert_eq!(sniff_version(b"no version here, avocado"), None);
    }

    #[test]
    fn zip_signature_inside_binary_is_not_a_container() {
        let mut v: Vec<u8> = (0..400u16).map(|i| (i % 7) as u8).collect();
        v.extend_from_slice(b"PK\x03\x04");
        assert_eq!(detect(&v).container, ContainerKind::Plain);
    }

    #[test]
    fn binary_and_cp1252() {
        let bin: Vec<u8> = (0..200u8).map(|i| i % 7).collect();
        assert_eq!(sniff_encoding(&bin), Encoding::Binary);
        assert_eq!(sniff_encoding(b"name=\"K\xf6ln\""), Encoding::Text(TextEncoding::Windows1252));
    }
}
