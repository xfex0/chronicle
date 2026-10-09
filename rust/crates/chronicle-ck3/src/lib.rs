//! Crusader Kings III adapter (MVP 1): read a save, import it into the campaign database.
//! Verified save layout: CK3 1.0.2 (games/ck3/SAVE_FORMAT.md).

pub mod import;
pub mod read;

pub use import::{Ck3Mapping, ImportError, ImportReport, import_world};
pub use read::{Ck3Error, Ck3Realm, Ck3World, read_ck3, read_gamestate};

/// Read a save and import it in one step.
pub fn import_save(db: &mut chronicle_db::CampaignDb, path: &std::path::Path) -> Result<ImportReport, ImportAnyError> {
    let world = read_ck3(path)?;
    Ok(import_world(db, &world, path)?)
}

#[derive(Debug, thiserror::Error)]
pub enum ImportAnyError {
    #[error(transparent)]
    Read(#[from] Ck3Error),
    #[error(transparent)]
    Import(#[from] ImportError),
}
