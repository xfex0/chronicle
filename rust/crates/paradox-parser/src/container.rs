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

pub fn detect(bytes: &[u8]) -> DetectedSave {
    let (header_line, payload_offset) = read_header(bytes);
    let payload = &bytes[payload_offset..];

    let (container, archive_offset) = if payload.starts_with(GZIP_MAGIC) {
        (ContainerKind::Gzip, Some(payload_offset))
    } else if payload.starts_with(ZSTD_MAGIC) {
        (ContainerKind::Zstd, Some(payload_offset))
    } else if let Some(pos) = find(payload, ZIP_MAGIC) {
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
    fn binary_and_cp1252() {
        let bin: Vec<u8> = (0..200u8).map(|i| i % 7).collect();
        assert_eq!(sniff_encoding(&bin), Encoding::Binary);
        assert_eq!(sniff_encoding(b"name=\"K\xf6ln\""), Encoding::Text(TextEncoding::Windows1252));
    }
}
