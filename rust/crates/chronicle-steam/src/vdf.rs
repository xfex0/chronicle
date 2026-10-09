//! Minimal Valve KeyValues (VDF/ACF) text parser: quoted or bare tokens, nested `{}`,
//! `//` comments, `[$PLATFORM]` conditionals (ignored), backslash escapes.

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Str(String),
    Obj(Vec<(String, Node)>),
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Obj(kv) => kv.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Node::Str(_) => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Node::Str(s) => Some(s),
            Node::Obj(_) => None,
        }
    }
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Node::str)
    }
    pub fn entries(&self) -> &[(String, Node)] {
        match self {
            Node::Obj(kv) => kv,
            Node::Str(_) => &[],
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VdfError {
    #[error("unexpected end of file")]
    Eof,
    #[error("unexpected '{0}' at byte {1}")]
    Unexpected(char, usize),
}

#[derive(Debug, PartialEq)]
enum Tok {
    Str(String),
    Open,
    Close,
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn skip(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            if self.i < self.s.len() && self.s[self.i] == b'[' {
                while self.i < self.s.len() && self.s[self.i] != b']' {
                    self.i += 1;
                }
                self.i = (self.i + 1).min(self.s.len());
                continue;
            }
            break;
        }
    }

    fn next(&mut self) -> Result<Option<Tok>, VdfError> {
        self.skip();
        let Some(&c) = self.s.get(self.i) else { return Ok(None) };
        match c {
            b'{' => {
                self.i += 1;
                Ok(Some(Tok::Open))
            }
            b'}' => {
                self.i += 1;
                Ok(Some(Tok::Close))
            }
            b'"' => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    let Some(&b) = self.s.get(self.i) else { return Err(VdfError::Eof) };
                    self.i += 1;
                    match b {
                        b'"' => break,
                        b'\\' => {
                            let Some(&e) = self.s.get(self.i) else { return Err(VdfError::Eof) };
                            self.i += 1;
                            out.push(match e {
                                b'n' => b'\n',
                                b't' => b'\t',
                                other => other, // \\ and \" and unknown escapes
                            });
                        }
                        _ => out.push(b),
                    }
                }
                Ok(Some(Tok::Str(String::from_utf8_lossy(&out).into_owned())))
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() && !self.s[self.i].is_ascii_whitespace() && !b"{}\"".contains(&self.s[self.i]) {
                    self.i += 1;
                }
                Ok(Some(Tok::Str(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())))
            }
        }
    }
}

fn parse_obj(lx: &mut Lexer<'_>, nested: bool) -> Result<Vec<(String, Node)>, VdfError> {
    let mut out = Vec::new();
    loop {
        match lx.next()? {
            None if nested => return Err(VdfError::Eof),
            None => return Ok(out),
            Some(Tok::Close) if nested => return Ok(out),
            Some(Tok::Close) => return Err(VdfError::Unexpected('}', lx.i - 1)),
            Some(Tok::Open) => return Err(VdfError::Unexpected('{', lx.i - 1)),
            Some(Tok::Str(key)) => match lx.next()? {
                None => return Err(VdfError::Eof),
                Some(Tok::Str(v)) => out.push((key, Node::Str(v))),
                Some(Tok::Open) => {
                    let child = parse_obj(lx, true)?;
                    out.push((key, Node::Obj(child)));
                }
                Some(Tok::Close) => return Err(VdfError::Unexpected('}', lx.i - 1)),
            },
        }
    }
}

/// Parse a whole document into a root object.
pub fn parse(src: &str) -> Result<Node, VdfError> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lx = Lexer { s: src.as_bytes(), i: 0 };
    Ok(Node::Obj(parse_obj(&mut lx, false)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_and_escapes() {
        let doc = parse(r#"
            // comment
            "AppState"
            {
                "appid"      "1158310"
                "installdir" "Crusader Kings III"
                "path"       "D:\\SteamLibrary"
                "UserConfig" { "language" "english" }
            }
        "#)
        .unwrap();
        let app = doc.get("appstate").unwrap();
        assert_eq!(app.get_str("installdir"), Some("Crusader Kings III"));
        assert_eq!(app.get_str("path"), Some(r"D:\SteamLibrary"));
        assert_eq!(app.get("UserConfig").unwrap().get_str("language"), Some("english"));
    }

    #[test]
    fn errors() {
        assert_eq!(parse(r#""a" { "b" "c""#), Err(VdfError::Eof));
        assert!(parse(r#""a" "unterminated"#).is_err());
        assert!(parse("}").is_err());
    }
}
