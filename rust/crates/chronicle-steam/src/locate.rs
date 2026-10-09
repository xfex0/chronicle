//! Find the Steam root and its libraries.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::vdf;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SteamInstall {
    pub root: PathBuf,
    /// Every library folder (the root is always one of them), de-duplicated, in Steam's order.
    pub libraries: Vec<PathBuf>,
    /// Human-readable trace for the "View raw detection log" button.
    pub log: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppManifest {
    pub appid: u32,
    pub name: Option<String>,
    pub installdir: String,
    pub buildid: Option<String>,
    pub library: PathBuf,
}

impl AppManifest {
    pub fn install_path(&self) -> PathBuf {
        self.library.join("steamapps").join("common").join(&self.installdir)
    }
}

/// Candidate Steam roots, most authoritative first.
pub fn steam_root_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam") {
            if let Ok(p) = k.get_value::<String, _>("SteamPath") {
                out.push(PathBuf::from(p.replace('/', "\\")));
            }
        }
        for sub in [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"] {
            if let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(sub) {
                if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                    out.push(PathBuf::from(p));
                }
            }
        }
        out.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        out.push(PathBuf::from(r"C:\Program Files\Steam"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("Library/Application Support/Steam"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".steam/steam"));
        out.push(home.join(".local/share/Steam"));
        out.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    out
}

fn looks_like_steam(root: &Path) -> bool {
    root.join("steamapps").is_dir()
}

/// Library paths from `libraryfolders.vdf`. Handles both formats:
/// new: `"0" { "path" "C:\\Steam" ... }`; old: `"1" "D:\\SteamLibrary"`.
pub fn parse_library_folders(src: &str) -> Result<Vec<PathBuf>, vdf::VdfError> {
    let doc = vdf::parse(src)?;
    let Some(top) = doc.entries().first().map(|(_, v)| v) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for (key, val) in top.entries() {
        if key.parse::<u32>().is_err() {
            continue; // e.g. "TimeNextStatsReport", "ContentStatsID"
        }
        let path = match val {
            vdf::Node::Str(p) => Some(p.as_str()),
            vdf::Node::Obj(_) => val.get_str("path"),
        };
        if let Some(p) = path {
            out.push(PathBuf::from(p));
        }
    }
    Ok(out)
}

pub fn parse_manifest(src: &str, library: &Path) -> Option<AppManifest> {
    let doc = vdf::parse(src).ok()?;
    let app = doc.get("AppState")?;
    Some(AppManifest {
        appid: app.get_str("appid")?.parse().ok()?,
        name: app.get_str("name").map(str::to_owned),
        installdir: app.get_str("installdir")?.to_owned(),
        buildid: app.get_str("buildid").map(str::to_owned),
        library: library.to_path_buf(),
    })
}

/// Locate Steam using the given candidate roots (first valid wins).
pub fn find_steam_in(candidates: &[PathBuf]) -> Option<SteamInstall> {
    let mut log = Vec::new();
    let root = candidates.iter().find(|c| {
        let ok = looks_like_steam(c);
        log.push(format!("{} candidate root: {}", if ok { "✓" } else { "✗" }, c.display()));
        ok
    })?;
    let mut libraries = vec![root.clone()];
    for vdf_path in [root.join("steamapps/libraryfolders.vdf"), root.join("config/libraryfolders.vdf")] {
        match std::fs::read_to_string(&vdf_path) {
            Ok(text) => match parse_library_folders(&text) {
                Ok(libs) => {
                    log.push(format!("✓ {} → {} libraries", vdf_path.display(), libs.len()));
                    libraries.extend(libs);
                    break;
                }
                Err(e) => log.push(format!("✗ {}: {e}", vdf_path.display())),
            },
            Err(_) => log.push(format!("· {} not found", vdf_path.display())),
        }
    }
    // De-duplicate case-insensitively (Windows paths), keep order.
    let mut seen = std::collections::HashSet::new();
    libraries.retain(|p| seen.insert(p.to_string_lossy().to_lowercase().replace('\\', "/")));
    Some(SteamInstall { root: root.clone(), libraries, log })
}

pub fn find_steam() -> Option<SteamInstall> {
    find_steam_in(&steam_root_candidates())
}

impl SteamInstall {
    pub fn manifest(&self, appid: u32) -> Option<AppManifest> {
        self.libraries.iter().find_map(|lib| {
            let p = lib.join("steamapps").join(format!("appmanifest_{appid}.acf"));
            parse_manifest(&std::fs::read_to_string(p).ok()?, lib)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEW_FORMAT: &str = r#"
"libraryfolders"
{
    "0" { "path" "C:\\Program Files (x86)\\Steam" "label" "" "apps" { "228980" "1" } }
    "1" { "path" "D:\\SteamLibrary" "apps" { "1158310" "123" } }
}"#;

    const OLD_FORMAT: &str = r#"
"LibraryFolders"
{
    "TimeNextStatsReport" "1600000000"
    "ContentStatsID" "-123"
    "1" "D:\\SteamLibrary"
    "2" "E:\\Games\\Steam"
}"#;

    #[test]
    fn both_libraryfolders_formats() {
        assert_eq!(parse_library_folders(NEW_FORMAT).unwrap(),
                   vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\SteamLibrary")]);
        assert_eq!(parse_library_folders(OLD_FORMAT).unwrap(),
                   vec![PathBuf::from(r"D:\SteamLibrary"), PathBuf::from(r"E:\Games\Steam")]);
    }

    #[test]
    fn manifest_and_install_path() {
        let m = parse_manifest(
            r#""AppState" { "appid" "1158310" "name" "Crusader Kings III" "installdir" "Crusader Kings III" "buildid" "42" }"#,
            Path::new("/lib"),
        )
        .unwrap();
        assert_eq!(m.appid, 1158310);
        assert_eq!(m.install_path(), Path::new("/lib/steamapps/common/Crusader Kings III"));
    }

    #[test]
    fn finds_fake_steam_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let lib2 = tmp.path().join("Lib2");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib2.join("steamapps/common/Crusader Kings III")).unwrap();
        let vdf_text = format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
            root.display().to_string().replace('\\', "\\\\"),
            lib2.display().to_string().replace('\\', "\\\\")
        );
        std::fs::write(root.join("steamapps/libraryfolders.vdf"), vdf_text).unwrap();
        std::fs::write(
            lib2.join("steamapps/appmanifest_1158310.acf"),
            r#""AppState" { "appid" "1158310" "installdir" "Crusader Kings III" }"#,
        )
        .unwrap();

        let steam = find_steam_in(&[tmp.path().join("nope"), root.clone()]).unwrap();
        assert_eq!(steam.root, root);
        assert_eq!(steam.libraries.len(), 2, "root listed twice must be de-duplicated: {:?}", steam.libraries);
        let m = steam.manifest(1158310).unwrap();
        assert!(m.install_path().is_dir());
        assert!(steam.manifest(394360).is_none());
    }
}
