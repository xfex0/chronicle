//! Time periods: who owned a territory when, and which government / ruler / dynasty / capital
//! an entity had when. A period is `[from, to)`; `to = None` means "still current".
//!
//! Writing follows the campaign rule "history only moves forward": a new period may start at
//! or after the start of the current one, never before it.

use chronicle_core::{ChronicleId, PartialDate};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::{CampaignDb, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    Government,
    Ruler,
    Dynasty,
    Capital,
    Religion,
    PrimaryCulture,
    Overlord,
}

impl Aspect {
    pub const ALL: [Aspect; 7] = [
        Self::Government,
        Self::Ruler,
        Self::Dynasty,
        Self::Capital,
        Self::Religion,
        Self::PrimaryCulture,
        Self::Overlord,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Government => "government",
            Self::Ruler => "ruler",
            Self::Dynasty => "dynasty",
            Self::Capital => "capital",
            Self::Religion => "religion",
            Self::PrimaryCulture => "primary_culture",
            Self::Overlord => "overlord",
        }
    }

    pub fn parse(s: &str) -> Option<Aspect> {
        Self::ALL.into_iter().find(|a| a.as_str() == s)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PeriodError {
    #[error("{subject}: new period at {date} starts before the current one ({current_from}); history only moves forward")]
    Backwards { subject: String, date: PartialDate, current_from: PartialDate },
    #[error("unknown aspect '{0}'")]
    UnknownAspect(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnershipPeriod {
    pub territory_id: String,
    pub owner_id: Option<String>,
    pub controller_id: Option<String>,
    pub from: PartialDate,
    pub to: Option<PartialDate>,
    pub game: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Period {
    pub entity_id: String,
    pub aspect: Aspect,
    pub value: Option<String>,
    pub value_entity_id: Option<String>,
    pub from: PartialDate,
    pub to: Option<PartialDate>,
    pub game: String,
    pub source: String,
}

/// Open ownership period: (row id, owner, from year, from month, from day).
type OpenOwnership = (i64, Option<String>, i32, Option<u8>, Option<u8>);
/// Open state period: (row id, value, value entity, from year, from month, from day).
type OpenPeriod = (i64, Option<String>, Option<String>, i32, Option<u8>, Option<u8>);

fn date_at(r: &Row<'_>, idx: usize) -> rusqlite::Result<Option<PartialDate>> {
    let year: Option<i32> = r.get(idx)?;
    Ok(year.map(|year| PartialDate { year, month: r.get(idx + 1).ok().flatten(), day: r.get(idx + 2).ok().flatten() }))
}

const OWNERSHIP_COLS: &str =
    "territory_id, owner_id, controller_id, from_year, from_month, from_day, to_year, to_month, to_day, game, source";

fn ownership_row(r: &Row<'_>) -> rusqlite::Result<OwnershipPeriod> {
    Ok(OwnershipPeriod {
        territory_id: r.get(0)?,
        owner_id: r.get(1)?,
        controller_id: r.get(2)?,
        from: date_at(r, 3)?.unwrap_or(PartialDate::year(0)),
        to: date_at(r, 6)?,
        game: r.get(9)?,
        source: r.get(10)?,
    })
}

const PERIOD_COLS: &str =
    "entity_id, aspect, value, value_entity_id, from_year, from_month, from_day, to_year, to_month, to_day, game, source";

fn period_row(r: &Row<'_>) -> rusqlite::Result<Period> {
    let aspect: String = r.get(1)?;
    Ok(Period {
        entity_id: r.get(0)?,
        aspect: Aspect::parse(&aspect).unwrap_or(Aspect::Government),
        value: r.get(2)?,
        value_entity_id: r.get(3)?,
        from: date_at(r, 4)?.unwrap_or(PartialDate::year(0)),
        to: date_at(r, 7)?,
        game: r.get(10)?,
        source: r.get(11)?,
    })
}

impl CampaignDb {
    /// Change the owner of a territory from `date`. Closes the open period, opens a new one.
    /// Setting the same owner again is a no-op.
    pub fn set_owner(
        &mut self,
        territory: &ChronicleId,
        owner: Option<&ChronicleId>,
        date: PartialDate,
        game: &str,
        source: &str,
    ) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        let open: Option<OpenOwnership> = tx
            .query_row(
                "SELECT id, owner_id, from_year, from_month, from_day FROM territory_ownership
                 WHERE territory_id = ?1 AND to_key IS NULL",
                [territory.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        if let Some((id, current_owner, y, m, d)) = open {
            let current_from = PartialDate { year: y, month: m, day: d };
            if date < current_from {
                return Err(PeriodError::Backwards { subject: territory.to_string(), date, current_from }.into());
            }
            if current_owner.as_deref() == owner.map(ChronicleId::as_str) {
                return Ok(());
            }
            tx.execute(
                "UPDATE territory_ownership SET to_year = ?1, to_month = ?2, to_day = ?3, to_key = ?4 WHERE id = ?5",
                params![date.year, date.month, date.day, date.sort_key(), id],
            )?;
        }
        tx.execute(
            "INSERT INTO territory_ownership(territory_id, owner_id, from_year, from_month, from_day, from_key, game, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                territory.as_str(),
                owner.map(ChronicleId::as_str),
                date.year,
                date.month,
                date.day,
                date.sort_key(),
                game,
                source
            ],
        )?;
        tx.execute(
            "UPDATE territories SET current_owner = ?1 WHERE entity_id = ?2",
            params![owner.map(ChronicleId::as_str), territory.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Owner of a territory at a date (None = unknown or unowned at that date).
    pub fn owner_at(&self, territory: &str, date: PartialDate) -> Result<Option<OwnershipPeriod>> {
        let k = date.sort_key();
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {OWNERSHIP_COLS} FROM territory_ownership
                     WHERE territory_id = ?1 AND from_key <= ?2 AND (to_key IS NULL OR to_key > ?2)"
                ),
                params![territory, k],
                ownership_row,
            )
            .optional()?)
    }

    pub fn ownership_history(&self, territory: &str) -> Result<Vec<OwnershipPeriod>> {
        let mut st = self.conn().prepare(&format!(
            "SELECT {OWNERSHIP_COLS} FROM territory_ownership WHERE territory_id = ?1 ORDER BY from_key"
        ))?;
        let rows = st.query_map([territory], ownership_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Territories owned by `owner` at `date`.
    pub fn territories_of(&self, owner: &str, date: PartialDate) -> Result<Vec<String>> {
        let mut st = self.conn().prepare(
            "SELECT territory_id FROM territory_ownership
             WHERE owner_id = ?1 AND from_key <= ?2 AND (to_key IS NULL OR to_key > ?2)
             ORDER BY territory_id",
        )?;
        let rows = st.query_map(params![owner, date.sort_key()], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Start a new period of `aspect` for `entity` (government form, ruler, dynasty, ...).
    #[allow(clippy::too_many_arguments)]
    pub fn set_period(
        &mut self,
        entity: &ChronicleId,
        aspect: Aspect,
        value: Option<&str>,
        value_entity: Option<&ChronicleId>,
        date: PartialDate,
        game: &str,
        source: &str,
    ) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        let open: Option<OpenPeriod> = tx
            .query_row(
                "SELECT id, value, value_entity_id, from_year, from_month, from_day FROM entity_periods
                 WHERE entity_id = ?1 AND aspect = ?2 AND to_key IS NULL",
                params![entity.as_str(), aspect.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()?;
        if let Some((id, cur_value, cur_entity, y, m, d)) = open {
            let current_from = PartialDate { year: y, month: m, day: d };
            if date < current_from {
                return Err(PeriodError::Backwards {
                    subject: format!("{entity} {}", aspect.as_str()),
                    date,
                    current_from,
                }
                .into());
            }
            if cur_value.as_deref() == value && cur_entity.as_deref() == value_entity.map(ChronicleId::as_str) {
                return Ok(());
            }
            tx.execute(
                "UPDATE entity_periods SET to_year = ?1, to_month = ?2, to_day = ?3, to_key = ?4 WHERE id = ?5",
                params![date.year, date.month, date.day, date.sort_key(), id],
            )?;
        }
        tx.execute(
            "INSERT INTO entity_periods(entity_id, aspect, value, value_entity_id, from_year, from_month, from_day,
                                        from_key, game, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entity.as_str(),
                aspect.as_str(),
                value,
                value_entity.map(ChronicleId::as_str),
                date.year,
                date.month,
                date.day,
                date.sort_key(),
                game,
                source
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn period_at(&self, entity: &str, aspect: Aspect, date: PartialDate) -> Result<Option<Period>> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {PERIOD_COLS} FROM entity_periods
                     WHERE entity_id = ?1 AND aspect = ?2 AND from_key <= ?3 AND (to_key IS NULL OR to_key > ?3)"
                ),
                params![entity, aspect.as_str(), date.sort_key()],
                period_row,
            )
            .optional()?)
    }

    pub fn periods(&self, entity: &str, aspect: Option<Aspect>) -> Result<Vec<Period>> {
        let mut st = self.conn().prepare(&format!(
            "SELECT {PERIOD_COLS} FROM entity_periods
             WHERE entity_id = ?1 AND (?2 IS NULL OR aspect = ?2) ORDER BY aspect, from_key"
        ))?;
        let rows = st.query_map(params![entity, aspect.map(Aspect::as_str)], period_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Continuous years the current value of `aspect` has held at `date`
    /// (e.g. "how long has this dynasty ruled?" for the rule "ancient dynasty").
    pub fn years_in_current_period(&self, entity: &str, aspect: Aspect, date: PartialDate) -> Result<Option<i32>> {
        Ok(self.period_at(entity, aspect, date)?.map(|p| date.years_since(&p.from)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DbError;
    use chronicle_core::registry::GameRegistry;
    use chronicle_core::{CampaignSettings, EntityKind};

    fn campaign() -> (tempfile::TempDir, CampaignDb) {
        let tmp = tempfile::tempdir().unwrap();
        let db = CampaignDb::create(
            &tmp.path().join("c"),
            "test",
            &CampaignSettings::new("ck3", 1),
            &GameRegistry::builtin().unwrap(),
        )
        .unwrap();
        (tmp, db)
    }

    fn d(s: &str) -> PartialDate {
        s.parse().unwrap()
    }

    #[test]
    fn ownership_timeline() {
        let (_tmp, mut db) = campaign();
        let kyiv = db.next_id(EntityKind::Territory).unwrap();
        let a = db.next_id(EntityKind::Country).unwrap();
        let b = db.next_id(EntityKind::Country).unwrap();

        db.set_owner(&kyiv, Some(&a), d("867.1.1"), "ck3", "demo").unwrap();
        db.set_owner(&kyiv, Some(&a), d("900"), "ck3", "demo").unwrap(); // same owner: no-op
        db.set_owner(&kyiv, Some(&b), d("1240.12.6"), "ck3", "demo").unwrap();
        db.set_owner(&kyiv, Some(&a), d("1362"), "ck3", "demo").unwrap();

        let h = db.ownership_history(kyiv.as_str()).unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].to, Some(d("1240.12.6")));
        assert_eq!(h[2].to, None);

        let at = |s: &str| db.owner_at(kyiv.as_str(), d(s)).unwrap().and_then(|p| p.owner_id);
        assert_eq!(at("1200").as_deref(), Some(a.as_str()));
        assert_eq!(at("1240.12.6").as_deref(), Some(b.as_str()), "period starts inclusive");
        assert_eq!(at("1300").as_deref(), Some(b.as_str()));
        assert_eq!(at("1400").as_deref(), Some(a.as_str()));
        assert_eq!(at("800"), None);
        assert_eq!(db.territories_of(b.as_str(), d("1300")).unwrap(), vec![kyiv.as_str().to_string()]);
    }

    #[test]
    fn history_only_moves_forward() {
        let (_tmp, mut db) = campaign();
        let t = db.next_id(EntityKind::Territory).unwrap();
        let a = db.next_id(EntityKind::Country).unwrap();
        db.set_owner(&t, Some(&a), d("1000"), "ck3", "demo").unwrap();
        let e = db.set_owner(&t, None, d("999"), "ck3", "demo").unwrap_err();
        assert!(matches!(e, DbError::Period(PeriodError::Backwards { .. })));
    }

    #[test]
    fn dynasty_duration_for_rules() {
        let (_tmp, mut db) = campaign();
        let c = db.next_id(EntityKind::Country).unwrap();
        let d1 = db.next_id(EntityKind::Dynasty).unwrap();
        let d2 = db.next_id(EntityKind::Dynasty).unwrap();
        db.set_period(&c, Aspect::Dynasty, None, Some(&d1), d("1021"), "ck3", "demo").unwrap();
        db.set_period(&c, Aspect::Dynasty, None, Some(&d2), d("1100"), "ck3", "demo").unwrap();
        db.set_period(&c, Aspect::Government, Some("feudal_monarchy"), None, d("1021"), "ck3", "demo").unwrap();

        assert_eq!(db.years_in_current_period(c.as_str(), Aspect::Dynasty, d("1450")).unwrap(), Some(350));
        let p = db.period_at(c.as_str(), Aspect::Dynasty, d("1050")).unwrap().unwrap();
        assert_eq!(p.value_entity_id.as_deref(), Some(d1.as_str()));
        assert_eq!(db.periods(c.as_str(), None).unwrap().len(), 3);
        assert_eq!(db.periods(c.as_str(), Some(Aspect::Government)).unwrap().len(), 1);
    }
}
