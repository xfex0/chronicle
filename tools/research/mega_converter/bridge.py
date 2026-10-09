"""Access to the Rust core (`paradox_core`, built with maturin). Optional at import time so
rules/formula work and tests run without a Rust toolchain."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

EXPECTED_ABI = "1"

try:  # pragma: no cover - depends on build environment
    import paradox_core as _core  # type: ignore[import-not-found]
except ImportError:  # pragma: no cover
    _core = None


class CoreUnavailable(RuntimeError):
    pass


def core() -> Any:
    if _core is None:
        raise CoreUnavailable(
            "paradox_core is not built. Run: cd rust/crates/python-bridge && maturin develop --release"
        )
    if getattr(_core, "__abi__", None) != EXPECTED_ABI:
        raise CoreUnavailable(f"paradox_core ABI {_core.__abi__!r} != expected {EXPECTED_ABI!r}; rebuild it")
    return _core


def detect_save(path: Path) -> dict[str, Any]:
    return json.loads(core().detect_save(str(path)))


def inspect(path: Path) -> list[dict[str, Any]]:
    return json.loads(core().inspect(str(path)))
