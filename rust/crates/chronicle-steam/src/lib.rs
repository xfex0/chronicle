//! Local game discovery.
//!
//! Steam root → `libraryfolders.vdf` → every library → `appmanifest_<appid>.acf` →
//! `installdir` → `<library>/steamapps/common/<installdir>` → fingerprint check.
//! No login, no Steam Web API. Steam is optional: games can be linked manually.

pub mod detect;
pub mod game_settings;
pub mod locate;
pub mod vdf;

pub use chronicle_core::InstallSource;
pub use game_settings::{SaveMode, SaveModeChange, SaveModeStatus, docs_dir, read_mode, set_mode};
pub use detect::{Detection, FingerprintResult, SaveFileInfo, detect_all, find_save_dir, list_save_files, verify_install_dir};
pub use locate::{SteamInstall, find_steam};
