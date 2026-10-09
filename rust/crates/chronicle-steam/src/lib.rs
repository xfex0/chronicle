//! Local game discovery.
//!
//! Steam root → `libraryfolders.vdf` → every library → `appmanifest_<appid>.acf` →
//! `installdir` → `<library>/steamapps/common/<installdir>` → fingerprint check.
//! No login, no Steam Web API. Steam is optional: games can be linked manually.

pub mod detect;
pub mod locate;
pub mod vdf;

pub use chronicle_core::InstallSource;
pub use detect::{Detection, FingerprintResult, detect_all, find_save_dir, verify_install_dir};
pub use locate::{SteamInstall, find_steam};
