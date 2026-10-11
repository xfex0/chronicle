//! Switch a game between binary and text saves by editing its own settings file
//! (HoI4: `Documents/Paradox Interactive/Hearts of Iron IV/settings.txt`, `save_as_binary`).
//!
//! Safe by construction: the file is backed up first, only the one `key=value` line is
//! changed (or appended when missing), everything else — including CRLF line endings — stays
//! byte for byte. The game must be closed: it rewrites its settings on exit.

use std::path::{Path, PathBuf};

use chronicle_core::TextSaveSetting;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    Binary,
    Text,
    /// The key is not in the file (the game uses its default, binary for HoI4).
    NotSet,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveModeStatus {
    pub settings_file: PathBuf,
    pub exists: bool,
    pub mode: SaveMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveModeChange {
    pub settings_file: PathBuf,
    pub backup: Option<PathBuf>,
    pub mode: SaveMode,
    pub changed: bool,
}

/// The game's documents folder from its save folder (".../Hearts of Iron IV/save games").
pub fn docs_dir(save_dir: &Path) -> PathBuf {
    if save_dir.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("save games")) {
        save_dir.parent().unwrap_or(save_dir).to_path_buf()
    } else {
        save_dir.to_path_buf()
    }
}

/// (line start, line end without the newline, value) of `key=value` at the top level.
fn find_key(text: &str, key: &str) -> Option<(usize, usize, String)> {
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let start = pos;
        pos += line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        let trimmed = content.trim_start();
        if !content.starts_with(|c: char| c.is_whitespace()) || trimmed.len() == content.len() {
            if let Some(rest) = trimmed.strip_prefix(key) {
                let rest = rest.trim_start();
                if let Some(value) = rest.strip_prefix('=') {
                    let value = value.trim().trim_matches('"').to_string();
                    return Some((start, start + content.len(), value));
                }
            }
        }
    }
    None
}

pub fn read_mode(docs: &Path, setting: &TextSaveSetting) -> SaveModeStatus {
    let settings_file = docs.join(&setting.file);
    let Ok(text) = std::fs::read_to_string(&settings_file) else {
        return SaveModeStatus { settings_file, exists: false, mode: SaveMode::NotSet };
    };
    let mode = match find_key(&text, &setting.key) {
        Some((_, _, v)) if v == setting.text_value => SaveMode::Text,
        Some((_, _, v)) if v == setting.binary_value => SaveMode::Binary,
        _ => SaveMode::NotSet,
    };
    SaveModeStatus { settings_file, exists: true, mode }
}

/// Set text (`text = true`) or binary saves. Backs the file up before changing it.
pub fn set_mode(docs: &Path, setting: &TextSaveSetting, text: bool) -> std::io::Result<SaveModeChange> {
    let settings_file = docs.join(&setting.file);
    let wanted = if text { &setting.text_value } else { &setting.binary_value };
    let mode = if text { SaveMode::Text } else { SaveMode::Binary };
    let original = if settings_file.exists() { std::fs::read_to_string(&settings_file)? } else { String::new() };
    if let Some((_, _, v)) = find_key(&original, &setting.key) {
        if &v == wanted {
            return Ok(SaveModeChange { settings_file, backup: None, mode, changed: false });
        }
    }
    let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
    let updated = match find_key(&original, &setting.key) {
        Some((start, end, _)) => {
            let indent: String = original[start..end].chars().take_while(|c| c.is_whitespace()).collect();
            format!("{}{indent}{}={wanted}{}", &original[..start], setting.key, &original[end..])
        }
        None => {
            let sep = if original.is_empty() || original.ends_with('\n') { "" } else { newline };
            format!("{original}{sep}{}={wanted}{newline}", setting.key)
        }
    };
    let mut backup = None;
    if settings_file.exists() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let b = docs.join(format!("{}.chronicle-{stamp}.bak", setting.file));
        std::fs::copy(&settings_file, &b)?;
        backup = Some(b);
    } else {
        std::fs::create_dir_all(docs)?;
    }
    std::fs::write(&settings_file, updated)?;
    Ok(SaveModeChange { settings_file, backup, mode, changed: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoi4() -> TextSaveSetting {
        TextSaveSetting {
            file: "settings.txt".into(),
            key: "save_as_binary".into(),
            text_value: "no".into(),
            binary_value: "yes".into(),
            verified: true,
        }
    }

    const SETTINGS: &str = "language=l_russian\r\nsave_as_binary=yes\r\ngraphics=\r\n{\r\n\tsize=\r\n\t{\r\n\t\tx=1920\r\n\t}\r\n}\r\n";

    #[test]
    fn switches_one_line_keeps_the_rest_and_backs_up() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.txt"), SETTINGS).unwrap();
        assert_eq!(read_mode(tmp.path(), &hoi4()).mode, SaveMode::Binary);
        let c = set_mode(tmp.path(), &hoi4(), true).unwrap();
        assert!(c.changed && c.backup.as_ref().unwrap().exists());
        let text = std::fs::read_to_string(tmp.path().join("settings.txt")).unwrap();
        assert_eq!(text, SETTINGS.replace("save_as_binary=yes", "save_as_binary=no"), "only that line changes, CRLF kept");
        assert_eq!(read_mode(tmp.path(), &hoi4()).mode, SaveMode::Text);
        let again = set_mode(tmp.path(), &hoi4(), true).unwrap();
        assert!(!again.changed && again.backup.is_none(), "nothing to do, no extra backup");
        set_mode(tmp.path(), &hoi4(), false).unwrap();
        assert_eq!(read_mode(tmp.path(), &hoi4()).mode, SaveMode::Binary);
    }

    #[test]
    fn appends_when_missing_and_ignores_nested_keys() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.txt"), "a=1\nblock={\n\tsave_as_binary=maybe\n}").unwrap();
        assert_eq!(read_mode(tmp.path(), &hoi4()).mode, SaveMode::NotSet);
        set_mode(tmp.path(), &hoi4(), true).unwrap();
        let text = std::fs::read_to_string(tmp.path().join("settings.txt")).unwrap();
        assert_eq!(text, "a=1\nblock={\n\tsave_as_binary=maybe\n}\nsave_as_binary=no\n");
        let missing = tempfile::tempdir().unwrap();
        assert!(!read_mode(missing.path(), &hoi4()).exists);
    }

    #[test]
    fn docs_from_save_folder() {
        assert_eq!(docs_dir(Path::new("/d/Hearts of Iron IV/save games")), PathBuf::from("/d/Hearts of Iron IV"));
    }
}
