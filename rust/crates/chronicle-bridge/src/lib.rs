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
pub mod sim;
pub mod state;

pub use config::{BridgeConfig, Vocabulary};
pub use design::{Decision, EmpireDesign, Reason, design_empire, validate_design};
pub use sim::{BridgeEvent, BridgeRun, simulate};
pub use state::{CivilizationState, INDICATORS, Ideology, StateError, presets};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeResult {
    pub seed: u64,
    pub start: CivilizationState,
    pub run: BridgeRun,
    pub design: EmpireDesign,
    /// Problems found by `validate_design` (empty = valid under the vocabulary rules).
    pub problems: Vec<String>,
}

/// Simulate and design with the built-in configuration.
pub fn run(start: &CivilizationState, seed: u64) -> Result<BridgeResult, StateError> {
    let cfg = BridgeConfig::builtin();
    let vocab = Vocabulary::builtin();
    start.validate()?;
    let run = simulate(start, seed, cfg);
    let mut design = design_empire(&run.final_state, &run.flags, cfg, vocab, start.coverage());
    let problems = validate_design(&design, vocab);
    design.set_validation(problems.is_empty());
    Ok(BridgeResult { seed, start: start.clone(), run, design, problems })
}
