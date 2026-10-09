//! # paradox-parser
//!
//! Game-agnostic reader for Paradox Clausewitz *text* data (saves and script files).
//!
//! Layers:
//! - [`container`] — detects how a save file is packaged (header line, zip, gzip, zstd,
//!   text vs binary). It never guesses game-internal field names.
//! - [`lexer`] — zero-copy tokenizer over `&[u8]`.
//! - [`index`] — fast first pass that records byte ranges of top-level keys without
//!   building an AST (enables lazy / per-section / parallel parsing of huge saves).
//! - [`parser`] — builds a borrowed [`ast::Container`] for a full document or a section.
//!
//! Binary (token-encoded) saves are intentionally *not* decoded here: see
//! [`container::Encoding::Binary`] and the `TokenResolver` design in `docs/ARCHITECTURE.md` §13.

pub mod ast;
pub mod container;
pub mod error;
pub mod index;
pub mod lexer;
pub mod parser;
pub mod source;

pub use ast::{Container, Entry, Operator, Scalar, Value};
pub use container::{ContainerKind, DetectedSave, Encoding, detect, sniff_version};
pub use error::{ParseError, Result};
pub use index::{SectionInfo, TopLevelIndex};
pub use parser::{ParseOptions, ParseOutput, parse, parse_with};
