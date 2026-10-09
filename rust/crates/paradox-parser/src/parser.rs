//! Recursive-descent parser producing a borrowed AST.

use crate::ast::{Container, Entry, Scalar, Value};
use crate::error::{ParseError, Result};
use crate::lexer::{Lexer, Token, TokenKind};

#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Tolerate stray `}` at top level and unclosed blocks at EOF (Paradox files contain both).
    pub lenient: bool,
    pub max_depth: usize,
    /// Bare words that introduce tagged blocks without `=`, e.g. `rgb { 1 2 3 }`.
    pub tagged_keywords: Vec<&'static str>,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            lenient: true,
            max_depth: 256,
            tagged_keywords: vec!["rgb", "hsv", "hsv360", "hex", "LIST"],
        }
    }
}

#[derive(Debug)]
pub struct ParseOutput<'a> {
    pub root: Container<'a>,
    /// Non-fatal problems found in lenient mode.
    pub warnings: Vec<String>,
}

pub fn parse(src: &[u8]) -> Result<ParseOutput<'_>> {
    parse_with(src, 0, &ParseOptions::default())
}

/// Parse `src` as a sequence of entries. `base` is the absolute offset of `src[0]`
/// in the original file (used when parsing a section slice from the index).
pub fn parse_with<'a>(src: &'a [u8], base: usize, opts: &ParseOptions) -> Result<ParseOutput<'a>> {
    let mut p = Parser { lexer: Lexer::with_base(src, base), peeked: None, opts, warnings: Vec::new() };
    let mut root = p.entries(0, None)?;
    root.offset = base;
    Ok(ParseOutput { root, warnings: p.warnings })
}

struct Parser<'a, 'o> {
    lexer: Lexer<'a>,
    peeked: Option<Token<'a>>,
    opts: &'o ParseOptions,
    warnings: Vec<String>,
}

fn scalar(t: Token<'_>) -> Scalar<'_> {
    Scalar { raw: t.text, quoted: t.kind == TokenKind::Quoted, offset: t.offset }
}

impl<'a> Parser<'a, '_> {
    fn bump(&mut self) -> Result<Option<Token<'a>>> {
        match self.peeked.take() {
            Some(t) => Ok(Some(t)),
            None => self.lexer.next_token(),
        }
    }

    fn peek(&mut self) -> Result<Option<Token<'a>>> {
        if self.peeked.is_none() {
            self.peeked = self.lexer.next_token()?;
        }
        Ok(self.peeked)
    }

    fn is_tag(&self, s: &Scalar<'a>) -> bool {
        !s.quoted && self.opts.tagged_keywords.iter().any(|k| k.as_bytes() == s.raw)
    }

    fn unexpected(t: Token<'a>, context: &'static str) -> ParseError {
        ParseError::UnexpectedToken { found: String::from_utf8_lossy(t.text).into_owned(), offset: t.offset, context }
    }

    /// Parse entries until the matching `}` (if `opened_at` is Some) or EOF (root).
    fn entries(&mut self, depth: usize, opened_at: Option<usize>) -> Result<Container<'a>> {
        let mut out = Container { entries: Vec::new(), offset: opened_at.unwrap_or(0) };
        loop {
            let Some(tok) = self.bump()? else {
                return match opened_at {
                    None => Ok(out),
                    Some(at) if self.opts.lenient => {
                        self.warnings.push(format!("unclosed block opened at byte {at}; closed at EOF"));
                        Ok(out)
                    }
                    Some(at) => Err(ParseError::UnexpectedEof { opened_at: at }),
                };
            };
            match tok.kind {
                TokenKind::Close => {
                    if opened_at.is_some() {
                        return Ok(out);
                    }
                    if self.opts.lenient {
                        self.warnings.push(format!("stray '}}' at byte {} ignored", tok.offset));
                        continue;
                    }
                    return Err(Self::unexpected(tok, "stray closing brace"));
                }
                TokenKind::Open => {
                    let c = self.block(depth + 1, tok.offset)?;
                    out.entries.push(Entry::Item(Value::Container(c)));
                }
                TokenKind::Op(_) => {
                    if self.opts.lenient {
                        self.warnings.push(format!("operator without key at byte {} ignored", tok.offset));
                        continue;
                    }
                    return Err(Self::unexpected(tok, "operator without key"));
                }
                TokenKind::Scalar | TokenKind::Quoted => {
                    let key = scalar(tok);
                    match self.peek()? {
                        Some(Token { kind: TokenKind::Op(op), .. }) => {
                            self.bump()?;
                            let value = self.value(depth)?;
                            out.entries.push(Entry::Pair { key, op, value });
                        }
                        Some(Token { kind: TokenKind::Open, offset, .. }) if self.is_tag(&key) => {
                            self.bump()?;
                            let body = self.block(depth + 1, offset)?;
                            out.entries.push(Entry::Item(Value::Tagged { tag: key, body }));
                        }
                        _ => out.entries.push(Entry::Item(Value::Scalar(key))),
                    }
                }
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value<'a>> {
        let Some(tok) = self.bump()? else {
            return Err(ParseError::UnexpectedEof { opened_at: self.lexer.position() });
        };
        match tok.kind {
            TokenKind::Open => Ok(Value::Container(self.block(depth + 1, tok.offset)?)),
            TokenKind::Scalar | TokenKind::Quoted => {
                let s = scalar(tok);
                if self.is_tag(&s) {
                    if let Some(Token { kind: TokenKind::Open, offset, .. }) = self.peek()? {
                        self.bump()?;
                        let body = self.block(depth + 1, offset)?;
                        return Ok(Value::Tagged { tag: s, body });
                    }
                }
                Ok(Value::Scalar(s))
            }
            TokenKind::Close | TokenKind::Op(_) => Err(Self::unexpected(tok, "expected value after operator")),
        }
    }

    fn block(&mut self, depth: usize, offset: usize) -> Result<Container<'a>> {
        if depth > self.opts.max_depth {
            return Err(ParseError::TooDeep { max: self.opts.max_depth, offset });
        }
        self.entries(depth, Some(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Operator;

    fn root(src: &str) -> Container<'_> {
        parse(src.as_bytes()).unwrap().root
    }

    #[test]
    fn pairs_nested_and_repeated_keys() {
        let r = root("a = 1\nb = { c = yes }\nwar = { id = 1 }\nwar = { id = 2 }");
        assert_eq!(r.get("a").unwrap().as_scalar().unwrap().as_i64(), Some(1));
        let b = r.get("b").unwrap().as_container().unwrap();
        assert_eq!(b.get("c").unwrap().as_scalar().unwrap().as_bool(), Some(true));
        assert_eq!(r.get_all("war").count(), 2);
    }

    #[test]
    fn arrays_and_mixed_containers() {
        let r = root("ids = { 1 2 3 }\nmixed = { x = 1 2 3 }");
        let ids = r.get("ids").unwrap().as_container().unwrap();
        assert!(ids.is_array());
        assert_eq!(ids.items().count(), 3);
        let mixed = r.get("mixed").unwrap().as_container().unwrap();
        assert!(!mixed.is_array());
        assert_eq!(mixed.pairs().count(), 1);
        assert_eq!(mixed.items().count(), 2);
    }

    #[test]
    fn array_of_objects() {
        let r = root("list = { { a = 1 } { a = 2 } }");
        let list = r.get("list").unwrap().as_container().unwrap();
        assert_eq!(list.items().count(), 2);
    }

    #[test]
    fn tagged_color() {
        let r = root("color = rgb { 10 20 30 }");
        match r.get("color").unwrap() {
            Value::Tagged { tag, body } => {
                assert_eq!(tag.raw, b"rgb");
                assert_eq!(body.items().count(), 3);
            }
            other => panic!("expected tagged, got {other:?}"),
        }
    }

    #[test]
    fn comparison_operators_in_pairs() {
        let r = root("trigger = { age >= 16 gold < 100 }");
        let t = r.get("trigger").unwrap().as_container().unwrap();
        let ops: Vec<_> = t.pairs().map(|(_, op, _)| op).collect();
        assert_eq!(ops, vec![Operator::Ge, Operator::Lt]);
    }

    #[test]
    fn quoted_keys_and_values() {
        let r = root("\"key with space\" = \"value\"");
        let (k, _, v) = r.pairs().next().unwrap();
        assert!(k.quoted);
        assert_eq!(k.as_str(), "key with space");
        assert_eq!(v.as_scalar().unwrap().as_str(), "value");
    }

    #[test]
    fn dates() {
        let r = root("date = 1066.9.15 bc = -304.1.1");
        assert_eq!(r.get("date").unwrap().as_scalar().unwrap().as_date(), Some((1066, 9, 15)));
        assert_eq!(r.get("bc").unwrap().as_scalar().unwrap().as_date(), Some((-304, 1, 1)));
    }

    #[test]
    fn lenient_mode_recovers() {
        let out = parse(b"a = { b = 1 } } c = 2").unwrap();
        assert_eq!(out.warnings.len(), 1);
        assert!(out.root.get("c").is_some());

        let out = parse(b"a = { b = 1").unwrap();
        assert_eq!(out.warnings.len(), 1);
    }

    #[test]
    fn strict_mode_errors() {
        let opts = ParseOptions { lenient: false, ..Default::default() };
        assert!(matches!(parse_with(b"a = { b = 1", 0, &opts), Err(ParseError::UnexpectedEof { opened_at: 4 })));
        assert!(parse_with(b"a = 1 }", 0, &opts).is_err());
    }

    #[test]
    fn depth_limit() {
        let opts = ParseOptions { max_depth: 3, ..Default::default() };
        assert!(matches!(parse_with(b"a={b={c={d={}}}}", 0, &opts), Err(ParseError::TooDeep { .. })));
    }

    #[test]
    fn missing_value_is_error() {
        assert!(parse(b"a = ").is_err());
        assert!(parse(b"a = }").is_err());
    }
}
