"""Plugin discovery.

A plugin is a directory with ``plugin.yaml``:

    id: better_population_conversion
    version: 0.1.0
    requires_converter: ">=0.1"
    rules: [rules/*.yaml]           # extra rule files
    formulas: [formulas.yaml]       # formula overlays (merge by name)
    hooks: hooks.py                 # optional: def register(registry) -> None

Python hooks execute arbitrary code — they are disabled unless the user enables the plugin
explicitly in campaign.yaml (``plugins.enabled``). Rules/formulas-only plugins are data and
safe to load.
"""

from __future__ import annotations

import importlib.util
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml


@dataclass
class PluginRegistry:
    actions: dict[str, Callable[..., None]] = field(default_factory=dict)
    features: dict[str, Callable[..., Any]] = field(default_factory=dict)
    validators: list[Callable[..., list[str]]] = field(default_factory=list)
    conversion_hooks: dict[str, list[Callable[..., None]]] = field(default_factory=dict)


@dataclass(frozen=True)
class Plugin:
    id: str
    root: Path
    version: str
    rule_files: tuple[Path, ...]
    formula_files: tuple[Path, ...]
    hooks: Path | None


def discover(plugins_dir: Path) -> list[Plugin]:
    out: list[Plugin] = []
    for manifest in sorted(Path(plugins_dir).glob("*/plugin.yaml")):
        root = manifest.parent
        doc = yaml.safe_load(manifest.read_text(encoding="utf-8")) or {}
        if "id" not in doc:
            raise ValueError(f"{manifest}: missing 'id'")
        rules = tuple(p for pat in doc.get("rules", []) for p in sorted(root.glob(pat)))
        formulas = tuple(root / f for f in doc.get("formulas", []))
        hooks = root / doc["hooks"] if doc.get("hooks") else None
        out.append(Plugin(doc["id"], root, str(doc.get("version", "0.0.0")), rules, formulas, hooks))
    return out


def load_hooks(plugin: Plugin, registry: PluginRegistry, enabled: set[str]) -> bool:
    if plugin.hooks is None or plugin.id not in enabled:
        return False
    spec = importlib.util.spec_from_file_location(f"mega_plugin_{plugin.id}", plugin.hooks)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load hooks for plugin {plugin.id}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.register(registry)
    return True
