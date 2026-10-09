"""campaign.yaml loader and validation (ТЗ §5)."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml


class ConfigError(ValueError):
    pass


CONVERSION_WEIGHT_KEYS = ("economy_weight", "population_weight", "military_weight",
                          "historical_weight", "prestige_weight")


@dataclass(frozen=True)
class CampaignConfig:
    raw: dict[str, Any]
    seed: int
    historical_divergence: float
    preserve_player_state: bool
    prevent_border_gore: bool
    allow_state_collapse: bool
    conversion_weights: dict[str, float]
    history_correction: dict[str, Any] = field(default_factory=dict)
    rule_files: tuple[str, ...] = ()
    formula_files: tuple[str, ...] = ()

    def section(self, name: str) -> dict[str, Any]:
        return dict(self.raw.get(name) or {})


def _unit(v: Any, name: str) -> float:
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not 0.0 <= float(v) <= 1.0:
        raise ConfigError(f"{name} must be a number in [0, 1], got {v!r}")
    return float(v)


def parse_campaign(doc: dict[str, Any]) -> CampaignConfig:
    camp = doc.get("campaign") or {}
    seed = camp.get("seed")
    if not isinstance(seed, int) or isinstance(seed, bool) or seed < 0:
        raise ConfigError("campaign.seed must be a non-negative integer (reproducibility, ТЗ §25)")
    conv = doc.get("conversion") or {}
    weights = {k: _unit(conv.get(k, 0.0), f"conversion.{k}") for k in CONVERSION_WEIGHT_KEYS}
    if abs(sum(weights.values()) - 1.0) > 1e-9:
        raise ConfigError(f"conversion weights must sum to 1, got {sum(weights.values()):.4f}")
    files = doc.get("files") or {}
    return CampaignConfig(
        raw=doc,
        seed=seed,
        historical_divergence=_unit(camp.get("historical_divergence", 0.35), "campaign.historical_divergence"),
        preserve_player_state=bool(camp.get("preserve_player_state", True)),
        prevent_border_gore=bool(camp.get("prevent_border_gore", True)),
        allow_state_collapse=bool(camp.get("allow_state_collapse", True)),
        conversion_weights=weights,
        history_correction=dict(doc.get("history_correction") or {}),
        rule_files=tuple(files.get("rules", [])),
        formula_files=tuple(files.get("formulas", [])),
    )


def load_campaign(path: Path) -> CampaignConfig:
    with Path(path).open(encoding="utf-8") as fh:
        return parse_campaign(yaml.safe_load(fh) or {})
