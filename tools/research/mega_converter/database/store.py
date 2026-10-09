"""SQLite access for campaign.db. Stdlib sqlite3; SQLAlchemy can replace this later if
query complexity warrants it — the public methods are the contract."""

from __future__ import annotations

import json
import sqlite3
from collections.abc import Iterable, Iterator, Mapping
from contextlib import contextmanager
from importlib import resources
from pathlib import Path
from typing import Any


class CampaignStore:
    def __init__(self, path: Path | str) -> None:
        self.path = str(path)
        self.conn = sqlite3.connect(self.path)
        self.conn.row_factory = sqlite3.Row
        self.conn.execute("PRAGMA foreign_keys = ON")
        if self.path != ":memory:":
            self.conn.execute("PRAGMA journal_mode = WAL")
        self.migrate()

    def close(self) -> None:
        self.conn.close()

    @contextmanager
    def tx(self) -> Iterator[sqlite3.Connection]:
        with self.conn:
            yield self.conn

    def migrate(self) -> None:
        sql = resources.files("mega_converter.database").joinpath("schema.sql").read_text(encoding="utf-8")
        with self.tx() as c:
            c.executescript(sql)

    @property
    def schema_version(self) -> int:
        return int(self.conn.execute("SELECT max(version) FROM schema_version").fetchone()[0])

    # --- campaigns / snapshots -----------------------------------------------------------

    def create_campaign(self, name: str, seed: int, config_yaml: str, converter_version: str) -> int:
        with self.tx() as c:
            cur = c.execute(
                "INSERT INTO campaigns(name, seed, config_yaml, converter_version) VALUES (?,?,?,?)",
                (name, seed, config_yaml, converter_version),
            )
            return int(cur.lastrowid or 0)

    def add_snapshot(self, campaign_id: int, *, game: str, kind: str, world_path: str, world_sha256: str,
                     uwm_schema_version: int, date: tuple[int, int, int] | None = None,
                     game_version: str | None = None, source_file: str | None = None,
                     source_sha256: str | None = None, parent_snapshot_id: int | None = None) -> int:
        y, m, d = date or (None, None, None)
        with self.tx() as c:
            cur = c.execute(
                """INSERT INTO snapshots(campaign_id, game, game_version, kind, date_year, date_month, date_day,
                       source_file, source_sha256, world_path, world_sha256, uwm_schema_version, parent_snapshot_id)
                   VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)""",
                (campaign_id, game, game_version, kind, y, m, d, source_file, source_sha256,
                 world_path, world_sha256, uwm_schema_version, parent_snapshot_id),
            )
            return int(cur.lastrowid or 0)

    def snapshots(self, campaign_id: int) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            "SELECT * FROM snapshots WHERE campaign_id=? ORDER BY date_year, date_month, date_day, id",
            (campaign_id,),
        )
        return [dict(r) for r in rows]

    # --- timeline ------------------------------------------------------------------------

    def add_events(self, campaign_id: int, events: Iterable[Mapping[str, Any]], origin: str,
                   snapshot_id: int | None = None) -> int:
        rows = [
            (campaign_id, snapshot_id, int(e["date"][0]), int(e["date"][1]), int(e["date"][2]), e["kind"],
             json.dumps(list(e.get("actors", [])), ensure_ascii=False), e.get("magnitude"),
             json.dumps(e.get("payload", {}), ensure_ascii=False, sort_keys=True), origin)
            for e in events
        ]
        with self.tx() as c:
            c.executemany(
                """INSERT INTO events(campaign_id, snapshot_id, year, month, day, kind, actors, magnitude,
                       payload, origin) VALUES (?,?,?,?,?,?,?,?,?,?)""",
                rows,
            )
        return len(rows)

    def events(self, campaign_id: int, kind: str | None = None) -> list[dict[str, Any]]:
        q = "SELECT * FROM events WHERE campaign_id=?"
        args: list[Any] = [campaign_id]
        if kind:
            q += " AND kind=?"
            args.append(kind)
        q += " ORDER BY year, month, day, id"
        return [
            {"date": [r["year"], r["month"], r["day"]], "kind": r["kind"], "actors": json.loads(r["actors"]),
             "magnitude": r["magnitude"], "payload": json.loads(r["payload"]), "origin": r["origin"]}
            for r in self.conn.execute(q, args)
        ]

    # --- AI cache ------------------------------------------------------------------------

    def ai_cached(self, campaign_id: int, task: str, input_sha256: str) -> dict[str, Any] | None:
        r = self.conn.execute(
            "SELECT output_json FROM ai_recommendations WHERE campaign_id=? AND task=? AND input_sha256=?",
            (campaign_id, task, input_sha256),
        ).fetchone()
        return json.loads(r[0]) if r else None

    def ai_store(self, campaign_id: int, entity_uid: str, task: str, input_sha256: str, model: str,
                 output: Mapping[str, Any], confidence: float | None) -> None:
        with self.tx() as c:
            c.execute(
                """INSERT OR IGNORE INTO ai_recommendations(campaign_id, entity_uid, task, input_sha256, model,
                       output_json, confidence) VALUES (?,?,?,?,?,?,?)""",
                (campaign_id, entity_uid, task, input_sha256, model,
                 json.dumps(output, ensure_ascii=False, sort_keys=True), confidence),
            )
