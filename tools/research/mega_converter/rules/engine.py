"""Rule evaluation.

Semantics (ARCHITECTURE.md §11.2):
- conditions see an immutable snapshot (features + attributes) — rules in the same phase
  cannot influence each other's conditions;
- effects are collected into a plan, then applied in one batch;
- modifiers are additive; results are clamped to field bounds;
- `set_value` conflicts: higher priority wins (rules are processed in priority order);
- every evaluation is traced for the explain report / `rule_applications` table.
"""

from __future__ import annotations

from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from mega_converter.rules.conditions import MISSING, describe, resolve_path
from mega_converter.rules.dsl import Action, Rule

ActionHandler = Callable[["EntityEffects", Any, Rule], None]


@dataclass(frozen=True)
class EntityContext:
    """What a rule can see about one entity."""

    uid: str
    scope: str
    features: Mapping[str, Any] = field(default_factory=dict)
    attrs: Mapping[str, Any] = field(default_factory=dict)
    flags: frozenset[str] = frozenset()

    def resolve(self, metric: str) -> Any:
        if metric == "uid":
            return self.uid
        v = resolve_path(self.features, metric)
        return v if v is not MISSING else resolve_path(self.attrs, metric)

    def has_flag(self, flag: str) -> bool:
        return flag in self.flags


@dataclass
class EntityEffects:
    modifiers: dict[str, float] = field(default_factory=dict)
    sets: dict[str, tuple[Any, str]] = field(default_factory=dict)  # field -> (value, rule)
    flags: set[str] = field(default_factory=set)
    recommendations: list[tuple[str, int, str]] = field(default_factory=list)  # (convert_as, prio, rule)
    protections: dict[str, Any] = field(default_factory=dict)
    events: list[dict[str, Any]] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    def is_empty(self) -> bool:
        return not (self.modifiers or self.sets or self.flags or self.recommendations
                    or self.protections or self.events)


@dataclass(frozen=True)
class RuleApplication:
    rule: str
    uid: str
    matched: bool
    condition: str
    missing_metrics: tuple[str, ...] = ()


@dataclass
class RuleOutcome:
    effects: dict[str, EntityEffects] = field(default_factory=dict)
    trace: list[RuleApplication] = field(default_factory=list)

    def for_entity(self, uid: str) -> EntityEffects:
        return self.effects.setdefault(uid, EntityEffects())

    def best_recommendation(self, uid: str) -> tuple[str, str] | None:
        """(convert_as, rule) with highest priority; ties → rule name order."""
        recs = self.effects.get(uid, EntityEffects()).recommendations
        if not recs:
            return None
        convert_as, _, rule = sorted(recs, key=lambda r: (-r[1], r[2]))[0]
        return convert_as, rule


# ---- built-in actions -------------------------------------------------------------------

def _add_modifier(fx: EntityEffects, args: Mapping[str, float], rule: Rule) -> None:
    for k, v in args.items():
        fx.modifiers[k] = fx.modifiers.get(k, 0.0) + float(v)


def _set_value(fx: EntityEffects, args: Mapping[str, Any], rule: Rule) -> None:
    for k, v in args.items():
        if k in fx.sets:
            fx.notes.append(f"set_value {k} from '{rule.name}' ignored: already set by '{fx.sets[k][1]}'")
        else:
            fx.sets[k] = (v, rule.name)


def _add_flag(fx: EntityEffects, args: str | list[str], rule: Rule) -> None:
    fx.flags.update([args] if isinstance(args, str) else args)


def _recommend(fx: EntityEffects, args: Any, rule: Rule) -> None:
    target = args["convert_as"] if isinstance(args, Mapping) else args
    fx.recommendations.append((str(target), rule.priority, rule.name))


def _protect(fx: EntityEffects, args: Mapping[str, Any], rule: Rule) -> None:
    for k, v in args.items():
        if k in fx.protections and k.startswith("limit_"):
            fx.protections[k] = min(fx.protections[k], v)  # the stricter limit wins
        else:
            fx.protections.setdefault(k, v)


def _emit_event(fx: EntityEffects, args: Mapping[str, Any], rule: Rule) -> None:
    fx.events.append({**args, "rule": rule.name})


BUILTIN_HANDLERS: dict[str, ActionHandler] = {
    "add_modifier": _add_modifier,
    "set_value": _set_value,
    "add_flag": _add_flag,
    "recommend_conversion": _recommend,
    "protect": _protect,
    "emit_event": _emit_event,
}


class RuleEngine:
    def __init__(self, rules: Iterable[Rule], extra_handlers: Mapping[str, ActionHandler] | None = None):
        self.rules = sorted((r for r in rules if r.enabled), key=lambda r: r.sort_key)
        self.handlers = {**BUILTIN_HANDLERS, **(extra_handlers or {})}

    def evaluate(self, entities: Sequence[EntityContext], phase: str) -> RuleOutcome:
        out = RuleOutcome()
        for ctx in sorted(entities, key=lambda e: e.uid):
            for rule in self.rules:
                if rule.phase != phase or rule.scope != ctx.scope:
                    continue
                missing: set[str] = set()
                matched = rule.condition.evaluate(ctx, missing)
                out.trace.append(
                    RuleApplication(rule.name, ctx.uid, matched, describe(rule.condition), tuple(sorted(missing)))
                )
                if matched:
                    fx = out.for_entity(ctx.uid)
                    for action in rule.actions:
                        self._apply(fx, action, rule)
        return out

    def _apply(self, fx: EntityEffects, action: Action, rule: Rule) -> None:
        self.handlers[action.kind](fx, action.args, rule)


DEFAULT_BOUNDS: dict[str, tuple[float, float]] = {
    "stability": (0.0, 1.0),
    "legitimacy": (0.0, 1.0),
    "prestige": (0.0, 1.0),
    "centralization": (0.0, 1.0),
    "bureaucracy": (0.0, 1.0),
    "regionalism": (0.0, 1.0),
    "trade_efficiency": (-1.0, 1.0),
}


def apply_effects(
    attrs: Mapping[str, Any],
    fx: EntityEffects,
    bounds: Mapping[str, tuple[float, float]] = DEFAULT_BOUNDS,
) -> dict[str, Any]:
    """Return a new attribute dict: sets first, then additive modifiers, then clamp."""
    out = dict(attrs)
    for k, (v, _) in fx.sets.items():
        out[k] = v
    for k, delta in fx.modifiers.items():
        out[k] = float(out.get(k, 0.0)) + delta
        if k in bounds:
            lo, hi = bounds[k]
            out[k] = min(hi, max(lo, out[k]))
    if fx.flags:
        out["flags"] = sorted(set(out.get("flags", [])) | fx.flags)
    return out
