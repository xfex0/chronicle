"""Pipeline stages that operate on UWM JSON. Parsing (Rust) produces that JSON; generation
(per target game) consumes the decisions."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any, Protocol

from mega_converter.conversion.state_transform import StructureInputs, TransformConfig, decide
from mega_converter.pipeline.context import PipelineContext
from mega_converter.rules import EntityContext, RuleEngine
from mega_converter.rules.engine import apply_effects
from mega_converter.simulation.history_features import compute_features


class Stage(Protocol):
    name: str

    def run(self, ctx: PipelineContext) -> None: ...


def _countries(ctx: PipelineContext) -> Mapping[str, dict[str, Any]]:
    return ctx.world.get("countries") or {}


def culture_fragmentation(ctx: PipelineContext, country: Mapping[str, Any]) -> float:
    """1 - share of the largest culture across the country's territories (population-weighted)."""
    terrs = ctx.world.get("territories") or {}
    totals: dict[str, float] = {}
    for tid in country.get("territories", []):
        pop = (terrs.get(tid) or {}).get("population") or {}
        size = float(pop.get("total", 0.0))
        for cul, share in (pop.get("culture_distribution") or {}).items():
            totals[cul] = totals.get(cul, 0.0) + size * float(share)
    s = sum(totals.values())
    return 0.0 if s <= 0 else 1.0 - max(totals.values()) / s


def vassal_autonomy(country: Mapping[str, Any]) -> float:
    subs = country.get("subjects") or []
    if not subs:
        return 0.0
    return sum(float(s.get("autonomy", 0.0)) for s in subs) / len(subs)


class HistoryStage:
    name = "history"

    def run(self, ctx: PipelineContext) -> None:
        for uid, c in sorted(_countries(ctx).items()):
            ctx.features[uid] = compute_features(uid, ctx.date, c, ctx.events)


class RulesStage:
    name = "rules"

    def __init__(self, phases: tuple[str, ...] = ("history", "pre_conversion")) -> None:
        self.phases = phases

    def run(self, ctx: PipelineContext) -> None:
        engine = RuleEngine(ctx.rules)
        countries = ctx.world.setdefault("countries", {})
        for phase in self.phases:
            # Each phase sees the results of the previous one (history → pre_conversion).
            entities = [
                EntityContext(uid, "country", ctx.features.get(uid, {}), c, frozenset(c.get("flags", [])))
                for uid, c in countries.items()
            ]
            outcome = engine.evaluate(entities, phase)
            ctx.rule_outcomes[phase] = outcome
            for uid, fx in outcome.effects.items():
                countries[uid] = apply_effects(countries[uid], fx)
            missing: dict[tuple[str, str], int] = {}
            for app in outcome.trace:
                for m in app.missing_metrics:
                    missing[(app.rule, m)] = missing.get((app.rule, m), 0) + 1
            for (rule, metric), n in sorted(missing.items()):
                ctx.notes.append(f"rule '{rule}': metric '{metric}' unavailable for {n} entities")


class StateTransformStage:
    name = "state_transform"

    def run(self, ctx: PipelineContext) -> None:
        cfg = TransformConfig(
            historical_divergence=ctx.config.historical_divergence,
            allow_state_collapse=ctx.config.allow_state_collapse,
        )
        pre = ctx.rule_outcomes.get("pre_conversion")
        for uid, c in sorted(_countries(ctx).items()):
            if c.get("overlord"):
                continue  # subjects follow their overlord's decision
            inputs = StructureInputs(
                centralization=float(c.get("centralization", 0.5)),
                vassal_autonomy=vassal_autonomy(c),
                civil_wars=int(ctx.features.get(uid, {}).get("civil_wars", 0)),
                culture_fragmentation=culture_fragmentation(ctx, c),
                legitimacy=float(c.get("legitimacy", 0.5)),
            )
            fx = pre.effects.get(uid) if pre else None
            ctx.decisions[uid] = decide(
                uid, inputs, cfg,
                override=(ctx.overrides.get(uid) or {}).get("convert_as"),
                protections=fx.protections if fx else None,
                rule_recommendation=pre.best_recommendation(uid) if pre else None,
            )
