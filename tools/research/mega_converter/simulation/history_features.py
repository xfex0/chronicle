"""History features: numeric facts derived from the campaign timeline, visible to rules.

Event shape (rows of the `events` table):
    {"date": [year, month, day], "kind": "civil_war", "actors": ["country:000042"], "payload": {...}}

Event kinds are the converter's own vocabulary (ARCHITECTURE.md §5.3), not game fields.
Plugins add features with ``@feature("name")``.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any

Date = tuple[int, int, int]


def years_between(later: Date, earlier: Date) -> int:
    y = later[0] - earlier[0]
    if (later[1], later[2]) < (earlier[1], earlier[2]):
        y -= 1
    return y


@dataclass(frozen=True)
class FeatureInput:
    uid: str
    at: Date
    attrs: Mapping[str, Any]
    events: Sequence[Mapping[str, Any]]          # all events, sorted by date
    entity_events: Sequence[Mapping[str, Any]]   # events where uid is an actor


FeatureFn = Callable[[FeatureInput], Any]
FEATURES: dict[str, FeatureFn] = {}


def feature(name: str) -> Callable[[FeatureFn], FeatureFn]:
    def deco(fn: FeatureFn) -> FeatureFn:
        if name in FEATURES:
            raise ValueError(f"feature '{name}' already registered")
        FEATURES[name] = fn
        return fn
    return deco


def _date(e: Mapping[str, Any]) -> Date:
    d = e["date"]
    return (int(d[0]), int(d[1]), int(d[2]))


def _count(kind: str) -> FeatureFn:
    return lambda x: sum(1 for e in x.entity_events if e["kind"] == kind and _date(e) <= x.at)


for _name, _kind in {
    "civil_wars": "civil_war",
    "capital_moves": "capital_moved",
    "revolutions": "revolution",
    "collapses": "state_collapsed",
    "major_wars": "major_war",
    "reforms": "reform",
}.items():
    FEATURES[_name] = _count(_kind)


@feature("years_since_founded")
def _years_since_founded(x: FeatureInput) -> int | None:
    created = [e for e in x.entity_events if e["kind"] == "state_created"]
    if created:
        return years_between(x.at, _date(created[0]))
    founded = x.attrs.get("founded")
    return years_between(x.at, tuple(founded)) if founded else None  # type: ignore[arg-type]


@feature("dynasty_age_years")
def _dynasty_age(x: FeatureInput) -> int | None:
    """Years the *current* ruling dynasty has held this state (continuous)."""
    dyn = x.attrs.get("ruling_dynasty")
    if not dyn:
        return None
    since: Date | None = None
    for e in x.entity_events:
        if e["kind"] == "dynasty_change" and _date(e) <= x.at:
            since = _date(e) if e.get("payload", {}).get("to") == dyn else None
    if since is None:
        created = [e for e in x.entity_events if e["kind"] == "state_created"]
        if created and created[0].get("payload", {}).get("dynasty") == dyn:
            since = _date(created[0])
    return years_between(x.at, since) if since else None


@feature("years_by_government")
def _years_by_government(x: FeatureInput) -> dict[str, int]:
    """{government_type: years}; rules can use e.g. `years_by_government.republic`."""
    changes = sorted(
        (_date(e), e.get("payload", {}).get("to")) for e in x.entity_events
        if e["kind"] == "government_changed" and _date(e) <= x.at
    )
    out: dict[str, int] = {}
    for (d, gov), nxt in zip(changes, [*changes[1:], (x.at, None)]):
        if gov:
            out[gov] = out.get(gov, 0) + years_between(nxt[0], d)
    return out


def compute_features(
    uid: str,
    at: Date,
    attrs: Mapping[str, Any],
    events: Sequence[Mapping[str, Any]],
    only: Sequence[str] | None = None,
) -> dict[str, Any]:
    ordered = sorted(events, key=_date)
    inp = FeatureInput(uid, at, attrs, ordered, [e for e in ordered if uid in e.get("actors", ())])
    names = sorted(only) if only else sorted(FEATURES)
    out: dict[str, Any] = {}
    for n in names:
        v = FEATURES[n](inp)
        if v is not None:
            out[n] = v
    return out
