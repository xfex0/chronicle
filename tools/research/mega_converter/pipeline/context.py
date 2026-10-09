from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from mega_converter.config import CampaignConfig
from mega_converter.conversion.formulas import FormulaSet
from mega_converter.conversion.state_transform import Decision
from mega_converter.rules import Rule, RuleOutcome


@dataclass
class PipelineContext:
    config: CampaignConfig
    world: dict[str, Any]                     # UWM JSON (from paradox_core or a fixture)
    from_game: str
    to_game: str
    events: list[dict[str, Any]] = field(default_factory=list)
    rules: list[Rule] = field(default_factory=list)
    formulas: FormulaSet = field(default_factory=dict)
    overrides: dict[str, dict[str, Any]] = field(default_factory=dict)
    features: dict[str, dict[str, Any]] = field(default_factory=dict)
    rule_outcomes: dict[str, RuleOutcome] = field(default_factory=dict)   # phase -> outcome
    decisions: dict[str, Decision] = field(default_factory=dict)
    validation: dict[str, Any] = field(default_factory=dict)
    warnings: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)       # informational (e.g. missing metrics)
    artifacts: dict[str, Any] = field(default_factory=dict)

    @property
    def date(self) -> tuple[int, int, int]:
        d = (self.world.get("meta") or {}).get("date") or {"year": 0, "month": 1, "day": 1}
        return (int(d["year"]), int(d["month"]), int(d["day"]))
