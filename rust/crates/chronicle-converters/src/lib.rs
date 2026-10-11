//! Install external converters from their official GitHub releases.
//!
//! Only repositories listed in `config/converters.yaml` are used, and only when the player
//! presses "Install" / "Update". Steps: latest release (GitHub API) → pick the Windows zip →
//! download → unpack safely (no paths outside the target folder) → find the program.
//! Each version goes into its own folder: `<base>/<repo name>/<tag>/`.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CATALOG_YAML: &str = include_str!("../../../../config/converters.yaml");
/// Upper bound for a download (converters with their mapping data are tens of MB).
const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ConverterError {
    #[error("network: {0}")]
    Network(String),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("catalog error: {0}")]
    Catalog(#[from] serde_yaml::Error),
    #[error("unknown converter route {0}")]
    UnknownRoute(String),
    #[error("the latest release has no Windows zip: {0}")]
    NoAsset(String),
    #[error("no converter program found in the release: {0}")]
    NoExecutable(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub route: String,
    pub name: String,
    pub repo: String,
    pub executables: Vec<String>,
}

#[derive(Deserialize)]
struct CatalogFile {
    converters: Vec<CatalogEntry>,
}

pub fn catalog() -> Result<Vec<CatalogEntry>, ConverterError> {
    Ok(serde_yaml::from_str::<CatalogFile>(CATALOG_YAML)?.converters)
}

pub fn entry(route: &str) -> Result<CatalogEntry, ConverterError> {
    catalog()?.into_iter().find(|e| e.route == route).ok_or_else(|| ConverterError::UnknownRoute(route.to_string()))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Installed {
    pub route: String,
    pub program: PathBuf,
    pub version: String,
    pub release_url: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(20)).build()
}

/// Latest published release of `owner/repo`.
pub fn latest_release(repo: &str) -> Result<Release, ConverterError> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    agent()
        .get(&url)
        .set("User-Agent", "Chronicle (Paradox mega campaign manager)")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| ConverterError::Network(e.to_string()))?
        .into_json::<Release>()
        .map_err(|e| ConverterError::Network(e.to_string()))
}

/// The Windows zip of a release: a .zip naming Windows first, otherwise the only/first .zip.
pub fn pick_asset(release: &Release) -> Option<&ReleaseAsset> {
    let zips: Vec<&ReleaseAsset> = release.assets.iter().filter(|a| a.name.to_lowercase().ends_with(".zip")).collect();
    let not_other_os = |a: &&ReleaseAsset| {
        let n = a.name.to_lowercase();
        !(n.contains("linux") || n.contains("mac") || n.contains("osx") || n.contains("source"))
    };
    zips.iter()
        .copied()
        .find(|a| {
            let n = a.name.to_lowercase();
            n.contains("win") || n.contains("windows")
        })
        .or_else(|| zips.iter().copied().find(not_other_os))
}

fn download(url: &str, dest: &Path) -> Result<(), ConverterError> {
    let resp = agent()
        .get(url)
        .set("User-Agent", "Chronicle (Paradox mega campaign manager)")
        .call()
        .map_err(|e| ConverterError::Network(e.to_string()))?;
    let mut file = std::fs::File::create(dest)?;
    std::io::copy(&mut resp.into_reader().take(MAX_DOWNLOAD), &mut file)?;
    Ok(())
}

/// Unpack a zip into `dir`. Entries that would land outside `dir` are skipped.
pub fn extract_zip(zip_path: &Path, dir: &Path) -> Result<usize, ConverterError> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    std::fs::create_dir_all(dir)?;
    let mut written = 0;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        let Some(rel) = f.enclosed_name() else { continue };
        let out = dir.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut f, &mut std::fs::File::create(&out)?)?;
        written += 1;
    }
    Ok(written)
}

/// The converter program inside an unpacked release: the first of `candidates` found anywhere
/// (shallowest first), else any `.exe` with "frontend" or "converter" in its name.
pub fn find_executable(dir: &Path, candidates: &[String]) -> Option<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in &entries {
            if p.is_file() {
                out.push(p.clone());
            }
        }
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, &mut files);
    let name = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    candidates
        .iter()
        .find_map(|c| files.iter().find(|p| name(p) == c.to_lowercase()).cloned())
        .or_else(|| {
            files
                .iter()
                .find(|p| {
                    let n = name(p);
                    n.ends_with(".exe") && (n.contains("frontend") || n.contains("converter"))
                })
                .cloned()
        })
}

fn safe_folder_name(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Download and unpack the latest release of the converter for `route` under `base`.
pub fn install(route: &str, base: &Path) -> Result<Installed, ConverterError> {
    let entry = entry(route)?;
    let release = latest_release(&entry.repo)?;
    let asset = pick_asset(&release).ok_or_else(|| ConverterError::NoAsset(release.html_url.clone()))?.clone();
    let repo_name = entry.repo.rsplit('/').next().unwrap_or(&entry.repo);
    let dir = base.join(safe_folder_name(repo_name)).join(safe_folder_name(&release.tag_name));
    std::fs::create_dir_all(&dir)?;
    let zip_path = dir.join(safe_folder_name(&asset.name));
    download(&asset.browser_download_url, &zip_path)?;
    extract_zip(&zip_path, &dir)?;
    let _ = std::fs::remove_file(&zip_path);
    let program = find_executable(&dir, &entry.executables).ok_or_else(|| ConverterError::NoExecutable(release.html_url.clone()))?;
    Ok(Installed { route: route.to_string(), program, version: release.tag_name, release_url: release.html_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset { name: name.into(), browser_download_url: format!("https://example.invalid/{name}"), size: 1 }
    }

    #[test]
    fn catalog_lists_vic3_to_hoi4() {
        let e = entry("victoria3->hoi4").unwrap();
        assert_eq!(e.repo, "ParadoxGameConverters/Vic3ToHoI4");
        assert!(entry("ck3->stellaris").is_err());
    }

    #[test]
    fn picks_the_windows_zip() {
        let r = Release {
            tag_name: "1.0".into(),
            html_url: String::new(),
            assets: vec![asset("Vic3ToHoI4-linux.zip"), asset("Vic3ToHoI4-win-x64.zip"), asset("notes.txt")],
        };
        assert_eq!(pick_asset(&r).unwrap().name, "Vic3ToHoI4-win-x64.zip");
        let only = Release { tag_name: "1".into(), html_url: String::new(), assets: vec![asset("Source code.zip"), asset("Converter.zip")] };
        assert_eq!(pick_asset(&only).unwrap().name, "Converter.zip");
        let none = Release { tag_name: "1".into(), html_url: String::new(), assets: vec![asset("readme.md")] };
        assert!(pick_asset(&none).is_none());
    }

    #[test]
    fn unpacks_safely_and_finds_the_program() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = tmp.path().join("c.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            w.start_file("Vic3ToHoI4/Converter/Vic3ToHoI4Converter.exe", opts).unwrap();
            w.write_all(b"MZ").unwrap();
            w.start_file("Vic3ToHoI4/ConverterFrontend.exe", opts).unwrap();
            w.write_all(b"MZ").unwrap();
            w.start_file("../escape.txt", opts).unwrap();
            w.write_all(b"no").unwrap();
            w.finish().unwrap();
        }
        let out = tmp.path().join("out");
        assert_eq!(extract_zip(&zip_path, &out).unwrap(), 2, "the ../ entry is skipped");
        assert!(!tmp.path().join("escape.txt").exists());
        let e = entry("victoria3->hoi4").unwrap();
        let exe = find_executable(&out, &e.executables).unwrap();
        assert!(exe.ends_with("ConverterFrontend.exe"), "the frontend is preferred: {exe:?}");
    }
}
