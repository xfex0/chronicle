"""Manual overrides (ТЗ §15) — persisted in campaign_overrides.yaml (source of truth) and
mirrored to the `manual_overrides` table for audit."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml

ALLOWED_FIELDS = {"convert_as", "name", "capital", "primary_culture", "religion", "government", "tag"}


@dataclass(frozen=True)
class Override:
    uid: str
    field: str
    value: Any
    note: str = ""


class OverrideStore:
    def __init__(self, path: Path) -> None:
        self.path = Path(path)
        self._data: dict[str, dict[str, Any]] = {}
        if self.path.exists():
            doc = yaml.safe_load(self.path.read_text(encoding="utf-8")) or {}
            self._data = {str(k): dict(v) for k, v in (doc.get("overrides") or {}).items()}

    def get(self, uid: str, field: str) -> Any:
        return self._data.get(uid, {}).get(field)

    def set(self, o: Override) -> None:
        if o.field not in ALLOWED_FIELDS:
            raise ValueError(f"field '{o.field}' cannot be overridden (allowed: {sorted(ALLOWED_FIELDS)})")
        self._data.setdefault(o.uid, {})[o.field] = o.value
        self.save()

    def remove(self, uid: str, field: str) -> None:
        self._data.get(uid, {}).pop(field, None)
        if uid in self._data and not self._data[uid]:
            del self._data[uid]
        self.save()

    def all(self) -> list[Override]:
        return [Override(u, f, v) for u in sorted(self._data) for f, v in sorted(self._data[u].items())]

    def save(self) -> None:
        tmp = self.path.with_suffix(".yaml.tmp")
        tmp.write_text(
            yaml.safe_dump({"version": 1, "overrides": self._data}, sort_keys=True, allow_unicode=True),
            encoding="utf-8",
        )
        tmp.replace(self.path)  # atomic on the same filesystem
