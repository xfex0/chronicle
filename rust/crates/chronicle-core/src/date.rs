//! Partial dates: games (and history) do not always give month/day.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `year` may be negative (BC). Missing month/day sort before any known month/day of that year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartialDate {
    pub year: i32,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

impl PartialDate {
    pub const fn ymd(year: i32, month: u8, day: u8) -> Self {
        Self { year, month: Some(month), day: Some(day) }
    }
    pub const fn year(year: i32) -> Self {
        Self { year, month: None, day: None }
    }
    fn key(&self) -> (i32, u8, u8) {
        (self.year, self.month.unwrap_or(0), self.day.unwrap_or(0))
    }

    /// Single sortable integer for SQL range queries: `year*10000 + month*100 + day`
    /// (unknown parts = 0). Ordering matches `Ord`, including BC years.
    pub fn sort_key(&self) -> i64 {
        self.year as i64 * 10_000 + self.month.unwrap_or(0) as i64 * 100 + self.day.unwrap_or(0) as i64
    }

    /// Whole years between two dates (later - earlier), ignoring unknown month/day.
    pub fn years_since(&self, earlier: &PartialDate) -> i32 {
        let mut y = self.year - earlier.year;
        if let (Some(m1), Some(m2)) = (self.month, earlier.month) {
            if (m1, self.day.unwrap_or(1)) < (m2, earlier.day.unwrap_or(1)) {
                y -= 1;
            }
        }
        y
    }
}

impl Ord for PartialDate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}
impl PartialOrd for PartialDate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid date '{0}' (expected Y, Y.M or Y.M.D; '-' prefix for BC)")]
pub struct DateError(pub String);

impl FromStr for PartialDate {
    type Err = DateError;
    /// Accepts `1337.4.1`, `1337-04-01`, `1337.4`, `1337`, `-304`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || DateError(s.to_string());
        let t = s.trim();
        let (neg, body) = match t.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, t),
        };
        let parts: Vec<&str> = body.split(['.', '-']).collect();
        if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
            return Err(err());
        }
        let y: i32 = parts[0].parse().map_err(|_| err())?;
        let month = parts.get(1).map(|m| m.parse::<u8>()).transpose().map_err(|_| err())?;
        let day = parts.get(2).map(|d| d.parse::<u8>()).transpose().map_err(|_| err())?;
        if month.is_some_and(|m| !(1..=12).contains(&m)) || day.is_some_and(|d| !(1..=31).contains(&d)) {
            return Err(err());
        }
        Ok(Self { year: if neg { -y } else { y }, month, day })
    }
}

impl fmt::Display for PartialDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.year)?;
        if let Some(m) = self.month {
            write!(f, ".{m}")?;
            if let Some(d) = self.day {
                write!(f, ".{d}")?;
            }
        }
        Ok(())
    }
}

impl Serialize for PartialDate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PartialDate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // YAML may hand us `1337` as a number or "1337.4.1" as a string.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Int(i64),
            Str(String),
        }
        match Raw::deserialize(d)? {
            Raw::Int(y) => Ok(PartialDate::year(y as i32)),
            Raw::Str(s) => s.parse().map_err(serde::de::Error::custom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_forms() {
        assert_eq!("1337.4.1".parse(), Ok(PartialDate::ymd(1337, 4, 1)));
        assert_eq!("1337-04-01".parse(), Ok(PartialDate::ymd(1337, 4, 1)));
        assert_eq!("-304".parse(), Ok(PartialDate::year(-304)));
        assert_eq!("1066.9".parse::<PartialDate>().unwrap().day, None);
        assert!("1337.13.1".parse::<PartialDate>().is_err());
        assert!("".parse::<PartialDate>().is_err());
        assert!("13x".parse::<PartialDate>().is_err());
    }

    #[test]
    fn ordering_with_partial_parts() {
        let y: PartialDate = "1337".parse().unwrap();
        let d: PartialDate = "1337.4.1".parse().unwrap();
        assert!(y < d);
        assert!(PartialDate::year(-304) < PartialDate::year(867));
    }

    #[test]
    fn sort_key_matches_ordering() {
        let dates: Vec<PartialDate> = ["-304.12.31", "-303.1.1", "-27", "867", "867.1.1", "1337.4.1", "1453"]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        for w in dates.windows(2) {
            assert!(w[0] < w[1]);
            assert!(w[0].sort_key() < w[1].sort_key(), "{} vs {}", w[0], w[1]);
        }
    }

    #[test]
    fn years_since() {
        assert_eq!(PartialDate::ymd(1337, 4, 1).years_since(&PartialDate::ymd(1021, 6, 1)), 315);
        assert_eq!(PartialDate::year(1337).years_since(&PartialDate::year(1021)), 316);
    }

    #[test]
    fn display_roundtrip() {
        for s in ["1337.4.1", "-27", "1066.9"] {
            assert_eq!(s.parse::<PartialDate>().unwrap().to_string(), s);
        }
    }

    #[test]
    fn serde_accepts_numbers_and_strings() {
        let a: PartialDate = serde_json::from_str("1453").unwrap();
        let b: PartialDate = serde_json::from_str("\"1453.1.1\"").unwrap();
        assert!(a < b);
        assert_eq!(serde_json::to_string(&b).unwrap(), "\"1453.1.1\"");
    }
}
