//! Confidence — composed, explainable, and a per-campaign policy the user edits in the UI.
//!
//! confidence = data_coverage × mapping_reliability, behind two gates:
//! - version gate: the adapter fully supports the source game version;
//! - validation gate: the value passed validation.
//! A closed gate sends the value to manual review regardless of the number. Gates (instead of
//! multiplying four factors) keep "everything is fine" values from drifting into the warning
//! band (0.9⁴ ≈ 0.66). All components are stored, so "Why?" can show them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConfidencePolicy {
    /// At or above: applied automatically.
    pub auto_min: f64,
    /// At or above (and below `auto_min`): applied with a warning. Below: manual review.
    pub warn_min: f64,
}

impl Default for ConfidencePolicy {
    fn default() -> Self {
        Self { auto_min: 0.80, warn_min: 0.50 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceDecision {
    Automatic,
    AutomaticWithWarning,
    ManualReview,
}

impl ConfidenceDecision {
    /// Value for `semantic_values.review_state`.
    pub fn review_state(self) -> &'static str {
        match self {
            Self::Automatic => "auto",
            Self::AutomaticWithWarning => "warning",
            Self::ManualReview => "pending_review",
        }
    }
}

/// The parts a confidence is made of. Stored next to every derived value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceComponents {
    /// Share of the expected source signals actually found (e.g. 3 of 4 → 0.75).
    pub data_coverage: f64,
    /// How well the formula is validated (calibrated on vanilla start states).
    pub mapping_reliability: f64,
    /// Gate: the adapter fully supports this game version.
    pub version_supported: bool,
    /// Gate: the value passed validation.
    pub validation_passed: bool,
}

impl ConfidenceComponents {
    pub fn new(found: usize, expected: usize, mapping_reliability: f64, version_supported: bool, validation_passed: bool) -> Self {
        let data_coverage = if expected == 0 { 0.0 } else { found.min(expected) as f64 / expected as f64 };
        Self { data_coverage, mapping_reliability: mapping_reliability.clamp(0.0, 1.0), version_supported, validation_passed }
    }

    pub fn gates_open(&self) -> bool {
        self.version_supported && self.validation_passed
    }

    /// The number shown to the user.
    pub fn confidence(&self) -> f64 {
        (self.data_coverage * self.mapping_reliability).clamp(0.0, 1.0)
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PolicyError {
    #[error("thresholds must be within [0, 1]")]
    OutOfRange,
    #[error("auto threshold ({auto}) must be greater than warning threshold ({warn})")]
    Order { auto: f64, warn: f64 },
}

impl ConfidencePolicy {
    pub fn validate(&self) -> Result<(), PolicyError> {
        let in_range = |v: f64| (0.0..=1.0).contains(&v);
        if !in_range(self.auto_min) || !in_range(self.warn_min) {
            return Err(PolicyError::OutOfRange);
        }
        if self.auto_min <= self.warn_min {
            return Err(PolicyError::Order { auto: self.auto_min, warn: self.warn_min });
        }
        Ok(())
    }

    /// Classify a bare confidence number (gates assumed open).
    pub fn classify(&self, confidence: f64) -> ConfidenceDecision {
        if confidence >= self.auto_min {
            ConfidenceDecision::Automatic
        } else if confidence >= self.warn_min {
            ConfidenceDecision::AutomaticWithWarning
        } else {
            ConfidenceDecision::ManualReview
        }
    }

    /// Classify with gates: a closed gate always means manual review.
    pub fn decide(&self, c: &ConfidenceComponents) -> ConfidenceDecision {
        if !c.gates_open() {
            return ConfidenceDecision::ManualReview;
        }
        self.classify(c.confidence())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_from_spec() {
        let p = ConfidencePolicy::default();
        assert_eq!(p.classify(0.93), ConfidenceDecision::Automatic);
        assert_eq!(p.classify(0.80), ConfidenceDecision::Automatic);
        assert_eq!(p.classify(0.63), ConfidenceDecision::AutomaticWithWarning);
        assert_eq!(p.classify(0.43), ConfidenceDecision::ManualReview);
    }

    #[test]
    fn components_and_gates() {
        let p = ConfidencePolicy::default();
        // 3 of 4 signals × 0.90 mapping = 0.675 → warning
        let c = ConfidenceComponents::new(3, 4, 0.90, true, true);
        assert!((c.confidence() - 0.675).abs() < 1e-12);
        assert_eq!(p.decide(&c), ConfidenceDecision::AutomaticWithWarning);
        // all good stays automatic (no 0.9^4 drift)
        assert_eq!(p.decide(&ConfidenceComponents::new(4, 4, 0.9, true, true)), ConfidenceDecision::Automatic);
        // unsupported version or failed validation → review, whatever the number
        assert_eq!(p.decide(&ConfidenceComponents::new(4, 4, 1.0, false, true)), ConfidenceDecision::ManualReview);
        assert_eq!(p.decide(&ConfidenceComponents::new(4, 4, 1.0, true, false)), ConfidenceDecision::ManualReview);
        assert_eq!(ConfidenceComponents::new(0, 0, 1.0, true, true).confidence(), 0.0);
    }

    #[test]
    fn user_policy_validation() {
        assert!(ConfidencePolicy { auto_min: 0.9, warn_min: 0.6 }.validate().is_ok());
        assert_eq!(
            ConfidencePolicy { auto_min: 0.5, warn_min: 0.5 }.validate(),
            Err(PolicyError::Order { auto: 0.5, warn: 0.5 })
        );
        assert_eq!(ConfidencePolicy { auto_min: 1.2, warn_min: 0.5 }.validate(), Err(PolicyError::OutOfRange));
    }
}
