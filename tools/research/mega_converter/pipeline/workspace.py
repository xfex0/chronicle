"""Campaign directory layout and the "never touch the original save" rule (ТЗ §26)."""

from __future__ import annotations

import hashlib
import os
import shutil
import stat
from dataclasses import dataclass
from pathlib import Path

SUBDIRS = ("input", "working", "output", "backup", "snapshots", "logs")


def sha256_file(path: Path, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with Path(path).open("rb") as fh:
        while block := fh.read(chunk):
            h.update(block)
    return h.hexdigest()


@dataclass(frozen=True)
class ImportedSave:
    original: Path
    input_copy: Path
    backup: Path
    sha256: str


@dataclass(frozen=True)
class Workspace:
    root: Path

    @classmethod
    def create(cls, root: Path) -> Workspace:
        root = Path(root)
        for d in SUBDIRS:
            (root / d).mkdir(parents=True, exist_ok=True)
        return cls(root)

    def __getattr__(self, name: str) -> Path:
        if name in SUBDIRS:
            return self.root / name
        raise AttributeError(name)

    @property
    def db_path(self) -> Path:
        return self.root / "campaign.db"

    @property
    def overrides_path(self) -> Path:
        return self.root / "campaign_overrides.yaml"

    def import_save(self, original: Path) -> ImportedSave:
        """Copy the user's save into backup/ (content-addressed) and input/ (read-only).
        The original is only ever opened for reading."""
        original = Path(original).resolve()
        digest = sha256_file(original)
        backup = self.root / "backup" / f"{digest}{original.suffix}"
        if not backup.exists():
            shutil.copy2(original, backup)
            os.chmod(backup, stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
        input_copy = self.root / "input" / f"{digest[:12]}_{original.name}"
        if not input_copy.exists():
            shutil.copy2(original, input_copy)
            os.chmod(input_copy, stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
        if sha256_file(input_copy) != digest:
            raise OSError(f"copy of {original} is corrupted")
        return ImportedSave(original, input_copy, backup, digest)
