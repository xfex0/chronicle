use thiserror::Error;

pub type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token {found:?} at byte {offset}: {context}")]
    UnexpectedToken {
        found: String,
        offset: usize,
        context: &'static str,
    },

    #[error("unexpected end of input (unclosed block opened at byte {opened_at})")]
    UnexpectedEof { opened_at: usize },

    #[error("unterminated quoted string starting at byte {offset}")]
    UnterminatedString { offset: usize },

    #[error("maximum nesting depth {max} exceeded at byte {offset}")]
    TooDeep { max: usize, offset: usize },

    #[error("binary (token-encoded) saves are not supported without a TokenResolver")]
    UnsupportedBinarySave,

    #[error("unsupported container: {0}")]
    UnsupportedContainer(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
