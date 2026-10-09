//! Top-level index: one lexer pass that records the byte span of every top-level value
//! without allocating an AST. Used to:
//! - show save structure (`paradox-inspect`) — the way we *discover* section names from
//!   real fixtures instead of guessing them;
//! - parse only the sections an adapter needs;
//! - parse large sections in parallel (spans are independent).

use std::collections::BTreeMap;

use crate::ast::Container;
use crate::error::Result;
use crate::lexer::{Lexer, TokenKind};
use crate::parser::{ParseOptions, ParseOutput, parse_with};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSpan {
    pub key: String,
    pub key_offset: usize,
    /// Absolute byte range of the value (for blocks: from `{` to `}` inclusive).
    pub value_start: usize,
    pub value_end: usize,
    pub is_block: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionInfo {
    pub key: String,
    pub occurrences: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Default)]
pub struct TopLevelIndex {
    pub spans: Vec<SectionSpan>,
}

impl TopLevelIndex {
    pub fn build(src: &[u8]) -> Result<Self> {
        let mut lx = Lexer::new(src);
        let mut spans = Vec::new();
        let mut depth: usize = 0;
        // (key, key_offset) waiting for its value at depth 0
        let mut pending_key: Option<(String, usize)> = None;
        let mut awaiting_value = false;
        // span being filled for a top-level block
        let mut open_block: Option<SectionSpan> = None;

        while let Some(tok) = lx.next_token()? {
            if depth > 0 {
                match tok.kind {
                    TokenKind::Open => depth += 1,
                    TokenKind::Close => {
                        depth -= 1;
                        if depth == 0 {
                            if let Some(mut s) = open_block.take() {
                                s.value_end = tok.offset + 1;
                                spans.push(s);
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
            match tok.kind {
                TokenKind::Op(_) if pending_key.is_some() => awaiting_value = true,
                TokenKind::Open => {
                    depth = 1;
                    if let (true, Some((key, key_offset))) = (awaiting_value, pending_key.take()) {
                        open_block = Some(SectionSpan {
                            key,
                            key_offset,
                            value_start: tok.offset,
                            value_end: tok.offset,
                            is_block: true,
                        });
                    }
                    awaiting_value = false;
                }
                TokenKind::Scalar | TokenKind::Quoted => {
                    if awaiting_value {
                        let (key, key_offset) = pending_key.take().expect("key present");
                        let len = tok.text.len() + if tok.kind == TokenKind::Quoted { 2 } else { 0 };
                        spans.push(SectionSpan {
                            key,
                            key_offset,
                            value_start: tok.offset,
                            value_end: tok.offset + len,
                            is_block: false,
                        });
                        awaiting_value = false;
                    } else {
                        pending_key = Some((String::from_utf8_lossy(tok.text).into_owned(), tok.offset));
                    }
                }
                // stray `}` or operator without key at top level: ignore (lenient)
                _ => {
                    pending_key = None;
                    awaiting_value = false;
                }
            }
        }
        // Unclosed final block: keep it, ending at EOF.
        if let Some(mut s) = open_block.take() {
            s.value_end = src.len();
            spans.push(s);
        }
        Ok(Self { spans })
    }

    /// Aggregate by key, sorted by total size descending (then key, for determinism).
    pub fn summary(&self) -> Vec<SectionInfo> {
        let mut agg: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for s in &self.spans {
            let e = agg.entry(s.key.as_str()).or_default();
            e.0 += 1;
            e.1 += s.value_end - s.value_start;
        }
        let mut v: Vec<_> = agg
            .into_iter()
            .map(|(k, (n, b))| SectionInfo { key: k.to_owned(), occurrences: n, total_bytes: b })
            .collect();
        v.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes).then_with(|| a.key.cmp(&b.key)));
        v
    }

    pub fn find<'s>(&'s self, key: &'s str) -> impl Iterator<Item = &'s SectionSpan> + 's {
        self.spans.iter().filter(move |s| s.key == key)
    }
}

/// Parse the *contents* of a block section (without its braces).
pub fn parse_section<'a>(src: &'a [u8], span: &SectionSpan, opts: &ParseOptions) -> Result<ParseOutput<'a>> {
    if span.is_block {
        let inner_start = span.value_start + 1;
        let inner_end = span.value_end.saturating_sub(1).max(inner_start).min(src.len());
        parse_with(&src[inner_start..inner_end], inner_start, opts)
    } else {
        Ok(ParseOutput { root: Container::default(), warnings: vec![format!("section '{}' is a scalar", span.key)] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "meta = { version = \"1.0\" }\nname = \"X\"\nitems = { { a = 1 } { a = 2 } }\nitems = { }\n";

    #[test]
    fn indexes_top_level_spans() {
        let idx = TopLevelIndex::build(DOC.as_bytes()).unwrap();
        let keys: Vec<_> = idx.spans.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, vec!["meta", "name", "items", "items"]);
        let name = idx.find("name").next().unwrap();
        assert_eq!(&DOC[name.value_start..name.value_end], "\"X\"");
        let meta = idx.find("meta").next().unwrap();
        assert_eq!(&DOC[meta.value_start..meta.value_end], "{ version = \"1.0\" }");
    }

    #[test]
    fn summary_aggregates_repeated_keys() {
        let idx = TopLevelIndex::build(DOC.as_bytes()).unwrap();
        let s = idx.summary();
        let items = s.iter().find(|i| i.key == "items").unwrap();
        assert_eq!(items.occurrences, 2);
    }

    #[test]
    fn section_parse_matches_offsets() {
        let idx = TopLevelIndex::build(DOC.as_bytes()).unwrap();
        let meta = idx.find("meta").next().unwrap();
        let out = parse_section(DOC.as_bytes(), meta, &ParseOptions::default()).unwrap();
        let v = out.root.get("version").unwrap().as_scalar().unwrap();
        assert_eq!(v.as_str(), "1.0");
        assert_eq!(&DOC.as_bytes()[v.offset..v.offset + 1], b"\"");
    }
}
