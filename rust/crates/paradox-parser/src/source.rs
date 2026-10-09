//! Read-only access to input files. Saves are memory-mapped when the `mmap` feature is on.

use std::ops::Deref;
use std::path::Path;

pub enum SourceBytes {
    #[cfg(feature = "mmap")]
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
}

impl Deref for SourceBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            #[cfg(feature = "mmap")]
            SourceBytes::Mapped(m) => m,
            SourceBytes::Owned(v) => v,
        }
    }
}

/// Open a file read-only. The pipeline copies saves into `input/` before parsing,
/// so the mapped file is never modified by the game while mapped.
#[allow(clippy::needless_return)]
pub fn open(path: &Path) -> std::io::Result<SourceBytes> {
    #[cfg(feature = "mmap")]
    {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() == 0 {
            return Ok(SourceBytes::Owned(Vec::new()));
        }
        // SAFETY: the file is opened read-only and lives in the campaign's `input/` copy;
        // concurrent external truncation would be UB, which the pipeline prevents by
        // never parsing the user's original save in place.
        let map = unsafe { memmap2::Mmap::map(&file)? };
        return Ok(SourceBytes::Mapped(map));
    }
    #[cfg(not(feature = "mmap"))]
    {
        return Ok(SourceBytes::Owned(std::fs::read(path)?));
    }
}
