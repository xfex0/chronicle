"""Advisor protocol + reproducibility wrapper.

Every recommendation is cached by (task, sha256(canonical input)). A repeated conversion
re-uses cached outputs, so seed + rules + cache ⇒ identical result even with an LLM.
The conversion engine decides whether to apply a recommendation; the GUI shows it with
confidence and reason.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from dataclasses import asdict, dataclass
from typing import Any, Protocol

TASKS = frozenset({
    "classify_state", "recommend_government", "recommend_successor_states", "flavour_text",
    "national_spirit_names", "description", "stellaris_ethics", "analyze_anomaly",
})


@dataclass(frozen=True)
class Recommendation:
    task: str
    entity_uid: str
    output: Mapping[str, Any]
    confidence: float
    reason: str
    model: str


class Advisor(Protocol):
    model: str

    def advise(self, task: str, entity_uid: str, payload: Mapping[str, Any]) -> Recommendation | None: ...


class NullAdvisor:
    """Default: AI disabled. The pipeline must work fully without AI."""

    model = "none"

    def advise(self, task: str, entity_uid: str, payload: Mapping[str, Any]) -> Recommendation | None:
        return None


class Cache(Protocol):
    def ai_cached(self, campaign_id: int, task: str, input_sha256: str) -> dict[str, Any] | None: ...
    def ai_store(self, campaign_id: int, entity_uid: str, task: str, input_sha256: str, model: str,
                 output: Mapping[str, Any], confidence: float | None) -> None: ...


def input_hash(task: str, payload: Mapping[str, Any]) -> str:
    canon = json.dumps({"task": task, "payload": payload}, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    return hashlib.sha256(canon.encode("utf-8")).hexdigest()


class CachedAdvisor:
    def __init__(self, inner: Advisor, cache: Cache, campaign_id: int) -> None:
        self.inner, self.cache, self.campaign_id = inner, cache, campaign_id
        self.model = inner.model

    def advise(self, task: str, entity_uid: str, payload: Mapping[str, Any]) -> Recommendation | None:
        if task not in TASKS:
            raise ValueError(f"unknown AI task '{task}'")
        key = input_hash(task, payload)
        hit = self.cache.ai_cached(self.campaign_id, task, key)
        if hit is not None:
            return Recommendation(**hit)
        rec = self.inner.advise(task, entity_uid, payload)
        if rec is not None:
            self.cache.ai_store(self.campaign_id, entity_uid, task, key, rec.model, asdict(rec), rec.confidence)
        return rec
