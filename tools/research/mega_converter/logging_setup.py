"""Structured logging: one JSON-lines file per stage under <campaign>/logs/ (ТЗ §20)."""

from __future__ import annotations

import json
import logging
from pathlib import Path

STAGES = ("parser", "mapping", "rules", "conversion", "validation", "generator", "ai", "pipeline")


class JsonFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        data = {
            "ts": self.formatTime(record, "%Y-%m-%dT%H:%M:%S"),
            "level": record.levelname,
            "stage": record.name.removeprefix("mega."),
            "msg": record.getMessage(),
        }
        extra = getattr(record, "data", None)
        if extra:
            data["data"] = extra
        return json.dumps(data, ensure_ascii=False)


def setup_logging(log_dir: Path | None, level: int = logging.INFO) -> None:
    root = logging.getLogger("mega")
    root.setLevel(level)
    root.handlers.clear()
    console = logging.StreamHandler()
    console.setLevel(logging.WARNING)
    console.setFormatter(logging.Formatter("%(levelname)s %(name)s: %(message)s"))
    root.addHandler(console)
    if log_dir is None:
        return
    log_dir.mkdir(parents=True, exist_ok=True)
    for stage in STAGES:
        h = logging.FileHandler(log_dir / f"{stage}.log", encoding="utf-8")
        h.setFormatter(JsonFormatter())
        logger = logging.getLogger(f"mega.{stage}")
        logger.handlers.clear()
        logger.addHandler(h)


def get_logger(stage: str) -> logging.Logger:
    if stage not in STAGES:
        raise ValueError(f"unknown log stage '{stage}'")
    return logging.getLogger(f"mega.{stage}")
