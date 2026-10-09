"""Pydantic models for validating UWM JSON coming from Rust or fixtures (subset)."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field

Unit = float  # [0, 1]; bounds enforced by the Rust validator for the full model


class _M(BaseModel):
    model_config = ConfigDict(extra="allow", frozen=True)


class GameDate(_M):
    year: int
    month: int = Field(ge=1, le=12)
    day: int = Field(ge=1, le=31)


class NativeRef(_M):
    game: str
    id: str


class Population(_M):
    total: float = Field(default=0.0, ge=0)
    culture_distribution: dict[str, float] = {}
    religion_distribution: dict[str, float] = {}
    classes: dict[str, float] = {}
    literacy: Unit = 0.0
    urbanization: Unit = 0.0


class SubjectLink(_M):
    subject: str
    kind: str = "vassal"
    autonomy: Unit = 0.0


class Country(_M):
    id: str
    name: str = ""
    capital: str | None = None
    stability: Unit = 0.0
    legitimacy: Unit = 0.0
    centralization: Unit = 0.0
    bureaucracy: Unit = 0.0
    prestige: Unit = 0.0
    primary_culture: str | None = None
    religion: str | None = None
    territories: list[str] = []
    overlord: str | None = None
    subjects: list[SubjectLink] = []
    ruling_dynasty: str | None = None
    founded: GameDate | None = None
    flags: list[str] = []
    native_refs: list[NativeRef] = []


class Territory(_M):
    id: str
    name: str = ""
    owner: str | None = None
    controller: str | None = None
    population: Population = Population()
    development: float = 0.0


class WorldMeta(_M):
    schema_version: int
    source_game: str | None = None
    source_game_version: str | None = None
    converter_version: str = ""
    date: GameDate | None = None
    seed: int = 0


class World(_M):
    meta: WorldMeta
    countries: dict[str, Country] = {}
    territories: dict[str, Territory] = {}
