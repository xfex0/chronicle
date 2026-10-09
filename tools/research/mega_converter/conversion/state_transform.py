"""State transformation (ТЗ §8): decide *how* a source state enters the target game.

Decision precedence (strongest first):
1. manual override (campaign_overrides.yaml);
2. history-correction protections (protect state / prevent collapse);
3. rule recommendation (highest priority);
4. fragmentation score from the state's structure and history.

AI recommendations are inputs to rules/the GUI, never applied here directly.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any

DEFAULT_WEIGHTS: dict[str, float] = {
    "decentralization": 0.20,   # 1 - centralization
    "vassal_autonomy": 0.20,
    "civil_wars": 0.20,         # saturates at civil_war_saturation
    "culture_fragmentation": 0.20,
    "illegitimacy": 0.20,       # 1 - legitimacy
}


@dataclass(frozen=True)
class StructureInputs:
    centralization: float
    vassal_autonomy: float
    civil_wars: int
    culture_fragmentation: float
    legitimacy: float


@dataclass(frozen=True)
class TransformConfig:
    historical_divergence: float = 0.35
    allow_state_collapse: bool = True
    civil_war_saturation: int = 8
    weights: Mapping[str, float] = field(default_factory=lambda: dict(DEFAULT_WEIGHTS))
    # Base thresholds at divergence 0.5; divergence shifts them (see effective_thresholds).
    federation_threshold: float = 0.45
    successor_threshold: float = 0.65

    def effective_thresholds(self) -> tuple[float, float]:
        """Lower divergence ⇒ more continuity ⇒ higher thresholds (harder to split)."""
        shift = (0.5 - self.historical_divergence) * 0.2
        return self.federation_threshold + shift, self.successor_threshold + shift


@dataclass(frozen=True)
class Decision:
    uid: str
    convert_as: str
    score: float
    source: str  # override | protection | rule:<name> | score
    reasons: tuple[str, ...]


def fragmentation_score(x: StructureInputs, cfg: TransformConfig) -> tuple[float, dict[str, float]]:
    terms = {
        "decentralization": 1.0 - x.centralization,
        "vassal_autonomy": x.vassal_autonomy,
        "civil_wars": min(x.civil_wars / cfg.civil_war_saturation, 1.0),
        "culture_fragmentation": x.culture_fragmentation,
        "illegitimacy": 1.0 - x.legitimacy,
    }
    total_w = sum(cfg.weights.values()) or 1.0
    contrib = {k: cfg.weights.get(k, 0.0) * v / total_w for k, v in terms.items()}
    return min(1.0, max(0.0, sum(contrib[k] for k in sorted(contrib)))), contrib


def decide(
    uid: str,
    x: StructureInputs,
    cfg: TransformConfig,
    *,
    override: str | None = None,
    protections: Mapping[str, Any] | None = None,
    rule_recommendation: tuple[str, str] | None = None,
) -> Decision:
    score, contrib = fragmentation_score(x, cfg)
    top = sorted(contrib.items(), key=lambda kv: (-kv[1], kv[0]))[:2]
    why = tuple(f"{k} contributes {v:.2f}" for k, v in top)

    if override:
        return Decision(uid, override, score, "override", ("manual override", *why))

    protections = protections or {}
    no_split = bool(protections.get("state") or protections.get("prevent_collapse")) or not cfg.allow_state_collapse

    if rule_recommendation:
        convert_as, rule = rule_recommendation
        if convert_as == "successor_states" and no_split:
            return Decision(uid, "federation", score, "protection",
                            (f"rule '{rule}' recommended split, blocked by protection", *why))
        return Decision(uid, convert_as, score, f"rule:{rule}", (f"recommended by rule '{rule}'", *why))

    fed_t, succ_t = cfg.effective_thresholds()
    if score >= succ_t:
        if no_split:
            return Decision(uid, "federation", score, "protection",
                            (f"score {score:.2f} ≥ {succ_t:.2f} but collapse is prevented", *why))
        return Decision(uid, "successor_states", score, "score", (f"score {score:.2f} ≥ {succ_t:.2f}", *why))
    if score >= fed_t:
        return Decision(uid, "federation", score, "score", (f"{fed_t:.2f} ≤ score {score:.2f} < {succ_t:.2f}", *why))
    return Decision(uid, "unified_state", score, "score", (f"score {score:.2f} < {fed_t:.2f}", *why))
