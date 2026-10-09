"""Condition tree for the rules DSL. Pure functions, no eval()."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass
from typing import Any, Protocol

MISSING: Any = object()


class Resolver(Protocol):
    def resolve(self, metric: str) -> Any: ...
    def has_flag(self, flag: str) -> bool: ...


def _num(v: Any) -> float | None:
    if isinstance(v, bool):
        return None
    return float(v) if isinstance(v, (int, float)) else None


def _gt(a: Any, b: Any) -> bool:
    x = _num(a)
    return x is not None and x > float(b)


def _lt(a: Any, b: Any) -> bool:
    x = _num(a)
    return x is not None and x < float(b)


def _ge(a: Any, b: Any) -> bool:
    x = _num(a)
    return x is not None and x >= float(b)


def _le(a: Any, b: Any) -> bool:
    x = _num(a)
    return x is not None and x <= float(b)


def _between(a: Any, b: Any) -> bool:
    x = _num(a)
    lo, hi = b
    return x is not None and float(lo) <= x <= float(hi)


COMPARATORS: dict[str, Callable[[Any, Any], bool]] = {
    "greater_than": _gt,
    "less_than": _lt,
    "at_least": _ge,
    "at_most": _le,
    "equals": lambda a, b: a == b,
    "not_equals": lambda a, b: a != b,
    "between": _between,
    "in": lambda a, b: a in b,
    "not_in": lambda a, b: a not in b,
}


@dataclass(frozen=True)
class Condition:
    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:  # pragma: no cover - abstract
        raise NotImplementedError


@dataclass(frozen=True)
class All(Condition):
    items: tuple[Condition, ...]

    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:
        # Evaluate all to collect every missing metric (better diagnostics), then combine.
        results = [c.evaluate(ctx, missing) for c in self.items]
        return all(results)


@dataclass(frozen=True)
class Any_(Condition):
    items: tuple[Condition, ...]

    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:
        results = [c.evaluate(ctx, missing) for c in self.items]
        return any(results)


@dataclass(frozen=True)
class Not(Condition):
    item: Condition

    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:
        return not self.item.evaluate(ctx, missing)


@dataclass(frozen=True)
class Compare(Condition):
    metric: str
    op: str
    value: Any

    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:
        if self.op == "exists":
            return (ctx.resolve(self.metric) is not MISSING) == bool(self.value)
        actual = ctx.resolve(self.metric)
        if actual is MISSING:
            missing.add(self.metric)
            return False
        return COMPARATORS[self.op](actual, self.value)


@dataclass(frozen=True)
class HasFlag(Condition):
    flag: str

    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:
        return ctx.has_flag(self.flag)


class Always(Condition):
    def evaluate(self, ctx: Resolver, missing: set[str]) -> bool:
        return True


def describe(c: Condition) -> str:
    """Human-readable form used in reports and the GUI."""
    match c:
        case All(items):
            return "(" + " and ".join(describe(i) for i in items) + ")"
        case Any_(items):
            return "(" + " or ".join(describe(i) for i in items) + ")"
        case Not(item):
            return f"not {describe(item)}"
        case Compare(metric, op, value):
            return f"{metric} {op} {value!r}"
        case HasFlag(flag):
            return f"has_flag {flag}"
        case _:
            return "always"


def resolve_path(data: Mapping[str, Any], dotted: str) -> Any:
    cur: Any = data
    for part in dotted.split("."):
        if isinstance(cur, Mapping) and part in cur:
            cur = cur[part]
        else:
            return MISSING
    return cur
