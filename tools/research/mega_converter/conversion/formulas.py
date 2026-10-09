"""Conversion formulas (ТЗ §9) — balance lives in YAML, never in code.

File shape (both forms accepted):

    ck3_to_eu4:
      province_tax:                 # long form
        normalized: true
        scale: 12
        inputs: {development: 0.4, control: 0.2, prosperity: 0.2, urbanization: 0.2}
      manpower:                     # short form: weights directly
        population: 0.6
        development: 0.2
        military_tradition: 0.2
      max_tax:
        expr: "clamp(base_tax * (1 + stability), 1, 30)"

Inputs are *UWM-derived normalized metrics* prepared by the mapper — not game field names.
"""

from __future__ import annotations

import ast
import math
import operator
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml


class FormulaError(ValueError):
    pass


_BINOPS = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul, ast.Div: operator.truediv}
_UNOPS = {ast.USub: operator.neg, ast.UAdd: operator.pos}
_FUNCS = {
    "min": min,
    "max": max,
    "abs": abs,
    "clamp": lambda v, lo, hi: min(hi, max(lo, v)),
    "sqrt": math.sqrt,
    "log1p": math.log1p,
}


class SafeExpr:
    """Arithmetic over whitelisted names/functions. Parsed once; no eval()."""

    def __init__(self, source: str) -> None:
        self.source = source
        try:
            self._tree = ast.parse(source, mode="eval").body
        except SyntaxError as e:
            raise FormulaError(f"bad expression {source!r}: {e.msg}") from e
        self.variables: set[str] = set()
        self._check(self._tree)

    def _check(self, n: ast.AST) -> None:
        match n:
            case ast.Constant(value=v) if isinstance(v, (int, float)) and not isinstance(v, bool):
                pass
            case ast.Name(id=name):
                if name in _FUNCS:
                    raise FormulaError(f"'{name}' is a function, not a variable")
                self.variables.add(name)
            case ast.BinOp(left=l, op=op, right=r) if type(op) in _BINOPS:
                self._check(l)
                self._check(r)
            case ast.UnaryOp(op=op, operand=o) if type(op) in _UNOPS:
                self._check(o)
            case ast.Call(func=ast.Name(id=fname), args=args, keywords=[]) if fname in _FUNCS:
                for a in args:
                    self._check(a)
            case _:
                raise FormulaError(f"disallowed syntax in {self.source!r}: {ast.dump(n)[:60]}")

    def __call__(self, env: Mapping[str, float]) -> float:
        return float(self._eval(self._tree, env))

    def _eval(self, n: ast.AST, env: Mapping[str, float]) -> Any:
        match n:
            case ast.Constant(value=v):
                return v
            case ast.Name(id=name):
                if name not in env:
                    raise FormulaError(f"missing input '{name}' for {self.source!r}")
                return env[name]
            case ast.BinOp(left=l, op=op, right=r):
                return _BINOPS[type(op)](self._eval(l, env), self._eval(r, env))
            case ast.UnaryOp(op=op, operand=o):
                return _UNOPS[type(op)](self._eval(o, env))
            case ast.Call(func=ast.Name(id=fname), args=args):
                return _FUNCS[fname](*(self._eval(a, env) for a in args))
        raise FormulaError("unreachable")  # pragma: no cover


@dataclass(frozen=True)
class Formula:
    name: str
    weights: Mapping[str, float] = field(default_factory=dict)
    normalized: bool = False
    scale: float = 1.0
    expr: SafeExpr | None = None

    @property
    def inputs(self) -> set[str]:
        return set(self.expr.variables) if self.expr else set(self.weights)

    def __call__(self, env: Mapping[str, float]) -> float:
        if self.expr is not None:
            return self.expr(env) * self.scale
        total = 0.0
        for k in sorted(self.weights):  # fixed order ⇒ identical float sums (matches Rust)
            if k not in env:
                raise FormulaError(f"formula '{self.name}': missing input '{k}'")
            total += self.weights[k] * float(env[k])
        return total * self.scale


def _parse_formula(name: str, node: Any) -> Formula:
    if not isinstance(node, Mapping):
        raise FormulaError(f"formula '{name}' must be a mapping")
    if "expr" in node:
        return Formula(name, normalized=False, scale=float(node.get("scale", 1.0)), expr=SafeExpr(str(node["expr"])))
    if "inputs" in node:
        weights, normalized, scale = node["inputs"], bool(node.get("normalized", True)), float(node.get("scale", 1.0))
    else:
        weights, normalized, scale = node, True, 1.0
    clean: dict[str, float] = {}
    for k, v in weights.items():
        if isinstance(v, bool) or not isinstance(v, (int, float)) or v < 0 or not math.isfinite(v):
            raise FormulaError(f"formula '{name}': weight '{k}' must be a finite number >= 0")
        clean[str(k)] = float(v)
    if normalized and abs(sum(clean.values()) - 1.0) > 1e-9:
        raise FormulaError(f"formula '{name}': weights sum to {sum(clean.values()):.6f}, expected 1")
    return Formula(name, clean, normalized, scale)


FormulaSet = dict[str, dict[str, Formula]]  # transition -> name -> formula


def parse_formulas(doc: Mapping[str, Any]) -> FormulaSet:
    out: FormulaSet = {}
    for transition, formulas in doc.items():
        if transition == "version":
            continue
        if not isinstance(formulas, Mapping):
            raise FormulaError(f"'{transition}' must map formula names to definitions")
        out[str(transition)] = {str(n): _parse_formula(f"{transition}.{n}", f) for n, f in formulas.items()}
    return out


def load_formulas(path: Path) -> FormulaSet:
    with Path(path).open(encoding="utf-8") as fh:
        return parse_formulas(yaml.safe_load(fh) or {})


def merge_formulas(base: FormulaSet, overlay: FormulaSet) -> FormulaSet:
    """Plugins/presets override individual formulas without copying whole files."""
    merged = {t: dict(fs) for t, fs in base.items()}
    for t, fs in overlay.items():
        merged.setdefault(t, {}).update(fs)
    return merged
