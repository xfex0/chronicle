//! Chronicle domain model. No I/O beyond parsing the embedded registry.

pub mod confidence;
pub mod date;
pub mod events;
pub mod ids;
pub mod platform;
pub mod registry;
pub mod settings;
pub mod transition;

pub use confidence::{ConfidenceComponents, ConfidenceDecision, ConfidencePolicy};
pub use date::PartialDate;
pub use events::{DatePrecision, EventEvidence, EventOrigin};
pub use ids::{ChronicleId, EntityKind};
pub use platform::{InstallSource, Platform};
pub use registry::{FormatSupport, GameDef, GameRegistry, RegistryError, SaveFormat, TransitionDef};
pub use settings::{CampaignSettings, TargetStart, TransitionMode};

pub const CHRONICLE_VERSION: &str = env!("CARGO_PKG_VERSION");
