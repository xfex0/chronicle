//! Borrowed AST for Clausewitz text. All slices point into the source buffer.

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Eq,       // =
    Lt,       // <
    Gt,       // >
    Le,       // <=
    Ge,       // >=
    Ne,       // !=
    Exists,   // ?=
}

impl Operator {
    pub fn as_str(self) -> &'static str {
        match self {
            Operator::Eq => "=",
            Operator::Lt => "<",
            Operator::Gt => ">",
            Operator::Le => "<=",
            Operator::Ge => ">=",
            Operator::Ne => "!=",
            Operator::Exists => "?=",
        }
    }
}

/// A raw scalar (bare or quoted). Decoding is lazy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scalar<'a> {
    pub raw: &'a [u8],
    pub quoted: bool,
    pub offset: usize,
}

impl<'a> Scalar<'a> {
    /// UTF-8 decode with replacement. Windows-1252 sources must be transcoded by the caller
    /// (see `container::Encoding`).
    pub fn as_str(&self) -> Cow<'a, str> {
        String::from_utf8_lossy(self.raw)
    }

    pub fn as_i64(&self) -> Option<i64> {
        std::str::from_utf8(self.raw).ok()?.parse().ok()
    }

    pub fn as_f64(&self) -> Option<f64> {
        std::str::from_utf8(self.raw).ok()?.parse().ok()
    }

    /// Clausewitz booleans are `yes` / `no`.
    pub fn as_bool(&self) -> Option<bool> {
        match self.raw {
            b"yes" => Some(true),
            b"no" => Some(false),
            _ => None,
        }
    }

    /// Dates are written as `Y.M.D` (year may be negative in some games). Returns (y, m, d).
    pub fn as_date(&self) -> Option<(i32, u8, u8)> {
        let s = std::str::from_utf8(self.raw).ok()?;
        let mut it = s.splitn(3, '.');
        let y: i32 = it.next()?.parse().ok()?;
        let m: u8 = it.next()?.parse().ok()?;
        let d: u8 = it.next()?.parse().ok()?;
        if (1..=12).contains(&m) && (1..=31).contains(&d) {
            Some((y, m, d))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Scalar(Scalar<'a>),
    Container(Container<'a>),
    /// `rgb { 1 2 3 }`, `hsv { ... }` and similar tagged blocks.
    Tagged { tag: Scalar<'a>, body: Container<'a> },
}

impl<'a> Value<'a> {
    pub fn as_scalar(&self) -> Option<&Scalar<'a>> {
        match self {
            Value::Scalar(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_container(&self) -> Option<&Container<'a>> {
        match self {
            Value::Container(c) => Some(c),
            Value::Tagged { body, .. } => Some(body),
            _ => None,
        }
    }
}

/// Clausewitz blocks can mix `key = value` pairs and bare items: `{ a = 1 2 3 }`.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry<'a> {
    Pair { key: Scalar<'a>, op: Operator, value: Value<'a> },
    Item(Value<'a>),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Container<'a> {
    pub entries: Vec<Entry<'a>>,
    /// Byte offset of the opening `{` (0 for the document root).
    pub offset: usize,
}

impl<'a> Container<'a> {
    /// First value for `key` (keys may repeat — repeated keys usually mean a list).
    pub fn get(&self, key: &str) -> Option<&Value<'a>> {
        self.get_all(key).next()
    }

    /// All values for `key`, in document order.
    pub fn get_all<'s>(&'s self, key: &'s str) -> impl Iterator<Item = &'s Value<'a>> + 's {
        self.entries.iter().filter_map(move |e| match e {
            Entry::Pair { key: k, value, .. } if k.raw == key.as_bytes() => Some(value),
            _ => None,
        })
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&Scalar<'a>, Operator, &Value<'a>)> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Pair { key, op, value } => Some((key, *op, value)),
            _ => None,
        })
    }

    pub fn items(&self) -> impl Iterator<Item = &Value<'a>> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Item(v) => Some(v),
            _ => None,
        })
    }

    pub fn is_array(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|e| matches!(e, Entry::Item(_)))
    }
}
