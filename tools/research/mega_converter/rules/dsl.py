"""Parse rule YAML into typed rules. Strict: unknown keys are errors with a location."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

from mega_converter.rules.conditions import (
    COMPARATORS,
    All,
    Always,
    Any_,
    Compare,
    Condition,
    HasFlag,
    Not,
)

SCOPES = frozenset({"world", "country", "territory", "dynasty", "culture", "religion", "character"})
PHASES = ("history", "pre_conversion", "conversion", "post_conversion")
CONVERT_AS = frozenset(
    {"unified_state", "federation", "confederation", "successor_states", "personal_union", "custom"}
)
PROTECT_KEYS = frozenset(
    {"state", "dynasty", "culture", "religion", "capital", "prevent_collapse", "historical_rival",
     "historical_alliance", "limit_assimilation", "limit_religious_conversion", "technology_floor"}
)
BUILTIN_ACTIONS = frozenset(
    {"add_modifier", "set_value", "add_flag", "recommend_conversion", "protect", "emit_event"}
)
RULE_KEYS = frozenset({"name", "description", "scope", "phase", "priority", "enabled", "if", "then"})


class RuleSyntaxError(ValueError):
    def __init__(self, message: str, where: str) -> None:
        super().__init__(f"{where}: {message}")
        self.where = where


@dataclass(frozen=True)
class Action:
    kind: str
    args: Any


@dataclass(frozen=True)
class Rule:
    name: str
    condition: Condition
    actions: tuple[Action, ...]
    scope: str = "country"
    phase: str = "pre_conversion"
    priority: int = 100
    enabled: bool = True
    description: str = ""
    source: str = "<memory>"

    @property
    def sort_key(self) -> tuple[int, str]:
        # Higher priority first; equal priority ordered by name ⇒ deterministic.
        return (-self.priority, self.name)


@dataclass
class ParseEnv:
    """Extension points: plugins can register extra actions."""

    extra_actions: set[str] = field(default_factory=set)

    @property
    def actions(self) -> frozenset[str]:
        return BUILTIN_ACTIONS | self.extra_actions


def parse_condition(node: Any, where: str) -> Condition:
    if node is None:
        return Always()
    if isinstance(node, list):
        return All(tuple(parse_condition(n, f"{where}[{i}]") for i, n in enumerate(node)))
    if not isinstance(node, Mapping):
        raise RuleSyntaxError(f"condition must be a mapping or list, got {type(node).__name__}", where)

    parts: list[Condition] = []
    for key, val in node.items():
        here = f"{where}.{key}"
        if key == "all":
            if not isinstance(val, list):
                raise RuleSyntaxError("'all' expects a list", here)
            parts.append(All(tuple(parse_condition(v, f"{here}[{i}]") for i, v in enumerate(val))))
        elif key == "any":
            if not isinstance(val, list):
                raise RuleSyntaxError("'any' expects a list", here)
            parts.append(Any_(tuple(parse_condition(v, f"{here}[{i}]") for i, v in enumerate(val))))
        elif key == "not":
            parts.append(Not(parse_condition(val, here)))
        elif key == "has_flag":
            parts.append(HasFlag(str(val)))
        elif isinstance(val, Mapping):
            for op, arg in val.items():
                if op not in COMPARATORS and op != "exists":
                    raise RuleSyntaxError(
                        f"unknown comparator '{op}' (allowed: {', '.join(sorted(COMPARATORS))}, exists)",
                        f"{here}.{op}",
                    )
                if op == "between" and not (isinstance(arg, list) and len(arg) == 2):
                    raise RuleSyntaxError("'between' expects [low, high]", f"{here}.{op}")
                if op in ("in", "not_in") and not isinstance(arg, list):
                    raise RuleSyntaxError(f"'{op}' expects a list", f"{here}.{op}")
                parts.append(Compare(str(key), str(op), arg))
        else:
            # `metric: value` shorthand for equals
            parts.append(Compare(str(key), "equals", val))
    return parts[0] if len(parts) == 1 else All(tuple(parts))


def _validate_action(kind: str, args: Any, where: str) -> None:
    if kind in ("add_modifier", "set_value"):
        if not isinstance(args, Mapping) or not args:
            raise RuleSyntaxError(f"'{kind}' expects a non-empty mapping field -> value", where)
        if kind == "add_modifier":
            for k, v in args.items():
                if isinstance(v, bool) or not isinstance(v, (int, float)):
                    raise RuleSyntaxError(f"modifier '{k}' must be numeric", f"{where}.{k}")
    elif kind == "add_flag":
        if not isinstance(args, (str, list)):
            raise RuleSyntaxError("'add_flag' expects a string or list", where)
    elif kind == "recommend_conversion":
        target = args.get("convert_as") if isinstance(args, Mapping) else args
        if target not in CONVERT_AS:
            raise RuleSyntaxError(f"convert_as must be one of {sorted(CONVERT_AS)}", where)
    elif kind == "protect":
        if not isinstance(args, Mapping):
            raise RuleSyntaxError("'protect' expects a mapping", where)
        unknown = set(args) - PROTECT_KEYS
        if unknown:
            raise RuleSyntaxError(f"unknown protect keys {sorted(unknown)}", where)
    elif kind == "emit_event":
        if not isinstance(args, Mapping) or "kind" not in args:
            raise RuleSyntaxError("'emit_event' expects a mapping with 'kind'", where)


def parse_actions(node: Any, where: str, env: ParseEnv) -> tuple[Action, ...]:
    if isinstance(node, Mapping):  # short form: then: {add_modifier: {...}, add_flag: x}
        node = [{k: v} for k, v in node.items()]
    if not isinstance(node, list) or not node:
        raise RuleSyntaxError("'then' must be a non-empty list or mapping of actions", where)
    out: list[Action] = []
    for i, item in enumerate(node):
        here = f"{where}[{i}]"
        if not isinstance(item, Mapping) or len(item) != 1:
            raise RuleSyntaxError("each action must be a single-key mapping", here)
        ((kind, args),) = item.items()
        if kind not in env.actions:
            raise RuleSyntaxError(f"unknown action '{kind}' (allowed: {', '.join(sorted(env.actions))})", here)
        _validate_action(kind, args, f"{here}.{kind}")
        out.append(Action(str(kind), args))
    return tuple(out)


def parse_rule(node: Any, where: str, env: ParseEnv, source: str) -> Rule:
    if not isinstance(node, Mapping):
        raise RuleSyntaxError("rule must be a mapping", where)
    unknown = set(node) - RULE_KEYS
    if unknown:
        raise RuleSyntaxError(f"unknown rule keys {sorted(unknown)}", where)
    name = node.get("name")
    if not isinstance(name, str) or not name:
        raise RuleSyntaxError("rule needs a non-empty 'name'", where)
    where = f"{where}({name})"
    scope = node.get("scope", "country")
    if scope not in SCOPES:
        raise RuleSyntaxError(f"scope must be one of {sorted(SCOPES)}", f"{where}.scope")
    phase = node.get("phase", "pre_conversion")
    if phase not in PHASES:
        raise RuleSyntaxError(f"phase must be one of {list(PHASES)}", f"{where}.phase")
    priority = node.get("priority", 100)
    if isinstance(priority, bool) or not isinstance(priority, int):
        raise RuleSyntaxError("priority must be an integer", f"{where}.priority")
    if "then" not in node:
        raise RuleSyntaxError("rule needs 'then'", where)
    return Rule(
        name=name,
        condition=parse_condition(node.get("if"), f"{where}.if"),
        actions=parse_actions(node["then"], f"{where}.then", env),
        scope=scope,
        phase=phase,
        priority=priority,
        enabled=bool(node.get("enabled", True)),
        description=str(node.get("description", "")),
        source=source,
    )


def parse_rules(doc: Any, source: str = "<memory>", env: ParseEnv | None = None) -> list[Rule]:
    env = env or ParseEnv()
    if doc is None:
        return []
    items = doc.get("rules", []) if isinstance(doc, Mapping) else doc
    if not isinstance(items, list):
        raise RuleSyntaxError("'rules' must be a list", source)
    rules = [parse_rule(r, f"{source}:rules[{i}]", env, source) for i, r in enumerate(items)]
    seen: set[str] = set()
    for r in rules:
        if r.name in seen:
            raise RuleSyntaxError(f"duplicate rule name '{r.name}'", source)
        seen.add(r.name)
    return rules


def load_rules(paths: Iterable[Path], env: ParseEnv | None = None) -> list[Rule]:
    """Load several files; names must be unique across all of them."""
    out: list[Rule] = []
    names: dict[str, str] = {}
    for p in sorted(Path(x) for x in paths):
        with p.open(encoding="utf-8") as fh:
            doc = yaml.safe_load(fh)
        for r in parse_rules(doc, str(p), env):
            if r.name in names:
                raise RuleSyntaxError(f"rule '{r.name}' already defined in {names[r.name]}", str(p))
            names[r.name] = str(p)
            out.append(r)
    return out
