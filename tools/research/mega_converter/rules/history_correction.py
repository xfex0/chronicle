"""History Correction System (ТЗ §7): campaign-level protections compiled into high-priority
rules. These constrain the conversion engine's decision space — they are not cheats."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from mega_converter.rules.dsl import Action, Rule
from mega_converter.rules.conditions import Always, Compare

PRIORITY = 1000

_PER_ENTITY = {
    "protect_states": ("country", "state"),
    "preserve_dynasties": ("dynasty", "dynasty"),
    "preserve_cultures": ("culture", "culture"),
    "preserve_religions": ("religion", "religion"),
    "preserve_capitals": ("country", "capital"),
}

_GLOBAL_COUNTRY = {
    "prevent_complete_collapse": "prevent_collapse",
    "limit_assimilation": "limit_assimilation",
    "limit_religious_conversion": "limit_religious_conversion",
    "prevent_unrealistic_technology_loss": "technology_floor",
}


def compile_history_correction(cfg: Mapping[str, Any] | None) -> list[Rule]:
    """``cfg`` is the ``history_correction`` block of campaign.yaml."""
    if not cfg:
        return []
    rules: list[Rule] = []
    for key, (scope, protect_key) in _PER_ENTITY.items():
        uids = cfg.get(key) or []
        if uids:
            rules.append(Rule(
                name=f"hc_{key}",
                condition=Compare("uid", "in", list(uids)),
                actions=(Action("protect", {protect_key: True}),),
                scope=scope,
                phase="pre_conversion",
                priority=PRIORITY,
                description=f"History correction: {key}",
                source="campaign.yaml#history_correction",
            ))
    for key, protect_key in _GLOBAL_COUNTRY.items():
        val = cfg.get(key)
        if val not in (None, False):
            rules.append(Rule(
                name=f"hc_{key}",
                condition=Always(),
                actions=(Action("protect", {protect_key: val}),),
                scope="country",
                phase="pre_conversion",
                priority=PRIORITY,
                description=f"History correction: {key}",
                source="campaign.yaml#history_correction",
            ))
    return rules
