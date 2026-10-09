//! Zero-copy tokenizer for Clausewitz text.
//!
//! Handles: `{ } = < > <= >= != ?=`, quoted strings with `\"` escapes, bare scalars,
//! `#` line comments, UTF-8 BOM. Bytes are never copied.

use crate::ast::Operator;
use crate::error::{ParseError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Open,
    Close,
    Op(Operator),
    Scalar,
    Quoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    /// For `Quoted`, the bytes *between* the quotes (escapes left as-is).
    pub text: &'a [u8],
    pub offset: usize,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    base: usize,
}

const BOM: &[u8] = b"\xEF\xBB\xBF";

impl<'a> Lexer<'a> {
    pub fn new(src: &'a [u8]) -> Self {
        Self::with_base(src, 0)
    }

    /// `base` is added to reported offsets (used when lexing a section slice).
    pub fn with_base(src: &'a [u8], base: usize) -> Self {
        let pos = if src.starts_with(BOM) { BOM.len() } else { 0 };
        Self { src, pos, base }
    }

    pub fn position(&self) -> usize {
        self.base + self.pos
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' | 0x0B | 0x0C => self.pos += 1,
                b'#' => {
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    #[inline]
    fn peek_at(&self, i: usize) -> Option<u8> {
        self.src.get(i).copied()
    }

    #[inline]
    fn is_scalar_end(&self, i: usize) -> bool {
        match self.src[i] {
            b' ' | b'\t' | b'\r' | b'\n' | 0x0B | 0x0C => true,
            b'{' | b'}' | b'=' | b'"' | b'#' | b'<' | b'>' => true,
            b'!' | b'?' => self.peek_at(i + 1) == Some(b'='),
            _ => false,
        }
    }

    #[inline]
    fn emit(&mut self, start: usize, len: usize, kind: TokenKind) -> Token<'a> {
        let src: &'a [u8] = self.src;
        self.pos = start + len;
        Token { kind, text: &src[start..start + len], offset: self.base + start }
    }

    pub fn next_token(&mut self) -> Result<Option<Token<'a>>> {
        self.skip_trivia();
        let start = self.pos;
        let Some(c) = self.peek_at(start) else {
            return Ok(None);
        };
        let offset = self.base + start;
        let next = self.peek_at(start + 1);
        let tok = match (c, next) {
            (b'{', _) => self.emit(start, 1, TokenKind::Open),
            (b'}', _) => self.emit(start, 1, TokenKind::Close),
            (b'<', Some(b'=')) => self.emit(start, 2, TokenKind::Op(Operator::Le)),
            (b'>', Some(b'=')) => self.emit(start, 2, TokenKind::Op(Operator::Ge)),
            (b'!', Some(b'=')) => self.emit(start, 2, TokenKind::Op(Operator::Ne)),
            (b'?', Some(b'=')) => self.emit(start, 2, TokenKind::Op(Operator::Exists)),
            // `==` appears in some script files.
            (b'=', Some(b'=')) => self.emit(start, 2, TokenKind::Op(Operator::Eq)),
            (b'=', _) => self.emit(start, 1, TokenKind::Op(Operator::Eq)),
            (b'<', _) => self.emit(start, 1, TokenKind::Op(Operator::Lt)),
            (b'>', _) => self.emit(start, 1, TokenKind::Op(Operator::Gt)),
            (b'"', _) => {
                let mut i = start + 1;
                loop {
                    match self.peek_at(i) {
                        None => return Err(ParseError::UnterminatedString { offset }),
                        Some(b'\\') => i += 2,
                        Some(b'"') => break,
                        Some(_) => i += 1,
                    }
                }
                let src: &'a [u8] = self.src;
                self.pos = i + 1;
                Token { kind: TokenKind::Quoted, text: &src[start + 1..i], offset }
            }
            _ => {
                let mut i = start;
                while i < self.src.len() && !self.is_scalar_end(i) {
                    i += 1;
                }
                if i == start {
                    // A lone `!` / `?` not followed by `=` is consumed as a 1-byte scalar.
                    i += 1;
                }
                let src: &'a [u8] = self.src;
                self.pos = i;
                Token { kind: TokenKind::Scalar, text: &src[start..i], offset }
            }
        };
        Ok(Some(tok))
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_token().transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(TokenKind, String)> {
        Lexer::new(src.as_bytes())
            .map(|t| t.unwrap())
            .map(|t| (t.kind, String::from_utf8_lossy(t.text).into_owned()))
            .collect()
    }

    #[test]
    fn basic_pairs_and_blocks() {
        let k = kinds("a=1 b = { c=\"x y\" }");
        assert_eq!(k.len(), 10);
        assert_eq!(k[0], (TokenKind::Scalar, "a".into()));
        assert_eq!(k[1], (TokenKind::Op(Operator::Eq), "=".into()));
        assert_eq!(k[8], (TokenKind::Quoted, "x y".into()));
    }

    #[test]
    fn operators() {
        let k = kinds("a<=1 b>=2 c!=3 d?=4 e<5 f>6");
        let ops: Vec<_> = k.iter().filter_map(|(k, _)| match k {
            TokenKind::Op(o) => Some(*o),
            _ => None,
        }).collect();
        assert_eq!(ops, vec![Operator::Le, Operator::Ge, Operator::Ne, Operator::Exists, Operator::Lt, Operator::Gt]);
    }

    #[test]
    fn comments_bom_and_dates() {
        let k = kinds("\u{FEFF}# header\ndate=867.1.1 # trailing\n");
        assert_eq!(k, vec![
            (TokenKind::Scalar, "date".into()),
            (TokenKind::Op(Operator::Eq), "=".into()),
            (TokenKind::Scalar, "867.1.1".into()),
        ]);
    }

    #[test]
    fn escaped_quote_and_hash_inside_string() {
        let k = kinds(r#"n="a \"b\" #c""#);
        assert_eq!(k[2], (TokenKind::Quoted, r#"a \"b\" #c"#.into()));
    }

    #[test]
    fn unterminated_string_is_error() {
        let mut l = Lexer::new(b"a=\"oops");
        l.next_token().unwrap();
        l.next_token().unwrap();
        assert!(matches!(l.next_token(), Err(ParseError::UnterminatedString { offset: 2 })));
    }
}
