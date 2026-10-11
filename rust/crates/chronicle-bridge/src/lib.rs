//! Modern Era Bridge (HoI4 → Stellaris).
//!
//! 1. A [`CivilizationState`] describes Earth at the end of a HoI4 campaign (14 indicators in
//!    0..1, number of power blocs, dominant ideology). It comes from the Semantic Registry once
//!    the HoI4 adapter exists, or from manual input / presets today.
//! 2. [`simulate`] runs 1948→2200 in decades, deterministically for a seed: technology and space
//!    growth, slow social drift, nuclear war, climate crises, conquest or peaceful unification.
//! 3. [`design_empire`] maps the final civilization to a Stellaris empire: authority, ethics
//!    (exactly 3 points), two civics and an origin — each with reasons and confidence.
//!
//! Balance: `config/bridge.yaml`. Stellaris vocabulary + rules: `games/stellaris/vocabulary.yaml`
//! (keys unverified until checked against the game). Research twin with identical results:
//! `tools/research/mega_converter/era_bridge.py`.

pub mod config;
pub mod design;
pub mod hoi4_import;
pub mod hoi4_save;
pub mod sim;
pub mod state;
pub mod stellaris_designs;
pub mod stellaris_mod;
pub mod stellaris_save;

pub use config::{BridgeConfig, Vocabulary};
pub use design::{Decision, EmpireDesign, Reason, design_empire, validate_design};
pub use hoi4_import::{Hoi4ImportError, Hoi4ImportReport, import_hoi4};
pub use hoi4_save::{Hoi4Civilization, Hoi4Error, Hoi4Mapping, Hoi4Summary, Hoi4World, read_hoi4, read_hoi4_bytes, to_civilization};
pub use sim::{BridgeEvent, BridgeRun, simulate, simulate_from};
pub use state::{CivilizationState, INDICATORS, Ideology, StateError, presets};
pub use stellaris_designs::{DesignsError, DesignsResult, stellaris_docs_from_save_dir, write_user_design};
pub use stellaris_mod::{ModError, ModOptions, ModResult, installed_version, mod_root_from_save_dir, suggested_name, write_mod};
pub use stellaris_save::{DesignComparison, ReadError, StellarisEmpire, compare, read_empire, read_empire_from_text};

use serde::Serialize;

/// Earliest bridge start: the first HoI4 bookmark.
pub const MIN_START_YEAR: i32 = 1936;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeResult {
    pub seed: u64,
    /// First simulated year (1948 by default, or the year the player chose / the HoI4 save's year).
    pub start_year: i32,
    pub start: CivilizationState,
    pub run: BridgeRun,
    pub design: EmpireDesign,
    /// Problems found by `validate_design` (empty = valid under the vocabulary rules).
    pub problems: Vec<String>,
}

/// Simulate and design with the built-in configuration, from the default year (1948).
pub fn run(start: &CivilizationState, seed: u64) -> Result<BridgeResult, StateError> {
    run_from(start, seed, None)
}

/// Same, from a chosen year: `MIN_START_YEAR` (1936) up to 10 years before the Stellaris start.
pub fn run_from(start: &CivilizationState, seed: u64, start_year: Option<i32>) -> Result<BridgeResult, StateError> {
    let cfg = BridgeConfig::builtin();
    let vocab = Vocabulary::builtin();
    start.validate()?;
    let year = start_year.unwrap_or(cfg.start_year);
    let max = cfg.end_year - cfg.step_years;
    if !(MIN_START_YEAR..=max).contains(&year) {
        return Err(StateError::StartYear { year, min: MIN_START_YEAR, max });
    }
    let run = simulate_from(start, seed, cfg, year);
    let mut design = design_empire(&run.final_state, &run.flags, cfg, vocab, start.coverage());
    let problems = validate_design(&design, vocab);
    design.set_validation(problems.is_empty());
    Ok(BridgeResult { seed, start_year: year, start: start.clone(), run, design, problems })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_year_bounds() {
        let p = presets()[0].1.clone();
        assert_eq!(run_from(&p, 1, Some(1936)).unwrap().start_year, 1936);
        assert_eq!(run(&p, 1).unwrap().start_year, 1948);
        assert!(matches!(run_from(&p, 1, Some(1935)), Err(StateError::StartYear { .. })));
        assert!(matches!(run_from(&p, 1, Some(2195)), Err(StateError::StartYear { .. })));
        assert!(run_from(&p, 1, Some(2190)).unwrap().problems.is_empty());
    }
}
