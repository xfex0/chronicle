//! Match registry games to installations and save folders.

use std::path::{Path, PathBuf};

use chronicle_core::InstallSource;
use chronicle_core::registry::{GameDef, GameKind, GameRegistry};
use serde::Serialize;

use crate::locate::SteamInstall;

/// Folder check against the registry fingerprint. Because most fingerprints are still
/// `verified: false`, a miss on an unverified fingerprint is NOT treated as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintResult {
    /// Matches a confirmed fingerprint.
    Verified,
    /// Matches an unconfirmed hint — almost certainly right.
    Likely,
    /// Folder exists, no hint matched, hints unconfirmed — cannot tell.
    Unverified,
    /// Folder exists but contradicts a confirmed fingerprint.
    Mismatch,
    NotFound,
}

impl FingerprintResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Likely => "likely",
            Self::Unverified => "unverified",
            Self::Mismatch => "mismatch",
            Self::NotFound => "not_found",
        }
    }
    pub fn acceptable(self) -> bool {
        !matches!(self, Self::Mismatch | Self::NotFound)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Detection {
    pub game_key: String,
    pub source: InstallSource,
    pub install_path: Option<PathBuf>,
    pub steam_app_id: Option<u32>,
    pub build_id: Option<String>,
    pub fingerprint: FingerprintResult,
    pub save_path: Option<PathBuf>,
}

pub fn verify_install_dir(game: &GameDef, dir: &Path) -> FingerprintResult {
    if !dir.is_dir() {
        return FingerprintResult::NotFound;
    }
    let Some(fp) = &game.install_fingerprint else { return FingerprintResult::Unverified };
    let hit = fp.candidates.iter().any(|c| dir.join(c).is_file());
    match (hit, fp.verified) {
        (true, true) => FingerprintResult::Verified,
        (true, false) => FingerprintResult::Likely,
        (false, true) => FingerprintResult::Mismatch,
        (false, false) => FingerprintResult::Unverified,
    }
}

/// Base folders that may contain `Paradox Interactive/...`, most likely first.
pub fn save_base_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    // Windows: Known Folder API — follows OneDrive / custom Documents redirection.
    if let Some(d) = dirs::document_dir() {
        v.push(d);
    }
    if let Some(home) = dirs::home_dir() {
        v.push(home.join("OneDrive").join("Documents"));
        v.push(home.join("Documents"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(d) = dirs::data_dir() {
        v.push(d); // ~/.local/share (native Linux builds)
    }
    let mut seen = std::collections::HashSet::new();
    v.retain(|p| seen.insert(p.clone()));
    v
}

pub fn find_save_dir_in(game: &GameDef, bases: &[PathBuf]) -> Option<PathBuf> {
    let hint = game.save_dir.as_ref()?;
    bases
        .iter()
        .flat_map(|b| hint.documents_subpath.iter().map(move |s| b.join(s)))
        .find(|p| p.is_dir())
}

pub fn find_save_dir(game: &GameDef) -> Option<PathBuf> {
    find_save_dir_in(game, &save_base_dirs())
}

pub fn detect_game(game: &GameDef, steam: Option<&SteamInstall>, save_bases: &[PathBuf]) -> Detection {
    let manifest = match (steam, game.steam_app_id) {
        (Some(s), Some(id)) => s.manifest(id),
        _ => None,
    };
    let (source, install_path, fingerprint) = match &manifest {
        Some(m) => {
            let p = m.install_path();
            let fp = verify_install_dir(game, &p);
            let source = if fp == FingerprintResult::NotFound { InstallSource::Missing } else { InstallSource::Steam };
            (source, Some(p), fp)
        }
        None => (InstallSource::Missing, None, FingerprintResult::NotFound),
    };
    Detection {
        game_key: game.key.clone(),
        source,
        install_path,
        steam_app_id: game.steam_app_id,
        build_id: manifest.and_then(|m| m.buildid),
        fingerprint,
        save_path: find_save_dir_in(game, save_bases),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SaveFileInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    /// Seconds since 1970 (file modification time).
    pub modified: u64,
}

/// Save files of a game in `dir` (and one level of subfolders — Stellaris keeps one folder per
/// empire), newest first. Files are only listed, never opened.
pub fn list_save_files(dir: &Path, extensions: &[String], limit: usize) -> Vec<SaveFileInfo> {
    fn walk(dir: &Path, exts: &[String], depth: u8, out: &mut Vec<SaveFileInfo>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let path = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if depth > 0 {
                    walk(&path, exts, depth - 1, out);
                }
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            if !exts.iter().any(|x| lower.ends_with(&x.to_lowercase())) {
                continue;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(SaveFileInfo { path, name, size: meta.len(), modified });
        }
    }
    let mut out = Vec::new();
    walk(dir, extensions, 1, &mut out);
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    out.truncate(limit);
    out
}

/// Detect every installable registry game (virtual stages are skipped).
pub fn detect_all(registry: &GameRegistry, steam: Option<&SteamInstall>) -> Vec<Detection> {
    let bases = save_base_dirs();
    registry
        .games
        .values()
        .filter(|g| g.kind == GameKind::Game)
        .map(|g| detect_game(g, steam, &bases))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locate::find_steam_in;

    fn reg() -> GameRegistry {
        GameRegistry::builtin().unwrap()
    }

    #[test]
    fn fingerprint_outcomes() {
        let tmp = tempfile::tempdir().unwrap();
        let ck3 = reg().game("ck3").unwrap().clone();
        assert_eq!(verify_install_dir(&ck3, &tmp.path().join("missing")), FingerprintResult::NotFound);
        assert_eq!(verify_install_dir(&ck3, tmp.path()), FingerprintResult::Unverified);
        let exe = tmp.path().join(&ck3.install_fingerprint.as_ref().unwrap().candidates[0]);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(verify_install_dir(&ck3, tmp.path()), FingerprintResult::Likely);
    }

    #[test]
    fn lists_saves_newest_first_including_subfolders() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("old.ck3"), b"x").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("empire")).unwrap();
        std::fs::write(tmp.path().join("empire/IRONMAN.SAV"), b"xy").unwrap();
        let ck3 = list_save_files(tmp.path(), &[".ck3".to_string()], 10);
        assert_eq!(ck3.len(), 1);
        assert_eq!(ck3[0].name, "old.ck3");
        let st = list_save_files(tmp.path(), &[".sav".to_string()], 10);
        assert_eq!((st.len(), st[0].size), (1, 2), "case-insensitive, one level deep");
        assert!(list_save_files(&tmp.path().join("missing"), &[".ck3".to_string()], 10).is_empty());
    }

    #[test]
    fn detects_steam_game_and_save_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        std::fs::create_dir_all(root.join("steamapps/common/Crusader Kings III")).unwrap();
        std::fs::write(
            root.join("steamapps/appmanifest_1158310.acf"),
            r#""AppState" { "appid" "1158310" "installdir" "Crusader Kings III" "buildid" "77" }"#,
        )
        .unwrap();
        let docs = tmp.path().join("Docs");
        std::fs::create_dir_all(docs.join("Paradox Interactive/Crusader Kings III/save games")).unwrap();

        let steam = find_steam_in(&[root]).unwrap();
        let r = reg();
        let d = detect_game(r.game("ck3").unwrap(), Some(&steam), &[docs.clone()]);
        assert_eq!(d.source, InstallSource::Steam);
        assert_eq!(d.build_id.as_deref(), Some("77"));
        assert!(d.fingerprint.acceptable());
        assert_eq!(d.save_path, Some(docs.join("Paradox Interactive/Crusader Kings III/save games")));

        let hoi = detect_game(r.game("hoi4").unwrap(), Some(&steam), &[docs]);
        assert_eq!((hoi.source, hoi.save_path), (InstallSource::Missing, None));
    }
}
