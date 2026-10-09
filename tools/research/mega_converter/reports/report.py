"""conversion_report.json / validation_report.json writers."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from mega_converter.conversion.state_transform import Decision


@dataclass
class ConversionReport:
    from_game: str
    to_game: str
    seed: int
    converter_version: str
    countries_created: list[str] = field(default_factory=list)
    countries_removed: list[str] = field(default_factory=list)
    states_split: list[str] = field(default_factory=list)
    states_merged: list[str] = field(default_factory=list)
    decisions: list[Decision] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    stats: dict[str, Any] = field(default_factory=dict)

    def to_json(self) -> str:
        return json.dumps(asdict(self), ensure_ascii=False, indent=2, sort_keys=True)

    def write(self, path: Path) -> None:
        Path(path).write_text(self.to_json(), encoding="utf-8")


def write_validation_report(report: dict[str, Any], path: Path) -> None:
    Path(path).write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True), encoding="utf-8")
