"""EU4 target model — an intermediate, typed representation of what the generator writes.

Only concepts that are well established in EU4 modding are modelled (3-letter country tags,
provinces with base tax/production/manpower, culture/religion, capital). Exact file syntax is
the generator's job and must be checked against the EU4 modding documentation.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

TAG_RE = re.compile(r"^[A-Z][A-Z0-9]{2}$")


@dataclass
class Eu4Country:
    tag: str
    source_uid: str
    name: str
    capital_province: int | None = None
    primary_culture: str | None = None
    religion: str | None = None
    government: str | None = None
    subjects: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not TAG_RE.match(self.tag):
            raise ValueError(f"invalid EU4 tag {self.tag!r}")


@dataclass
class Eu4Province:
    id: int
    owner: str | None
    base_tax: int
    base_production: int
    base_manpower: int
    culture: str | None = None
    religion: str | None = None


@dataclass
class Eu4World:
    start_date: tuple[int, int, int]
    countries: dict[str, Eu4Country] = field(default_factory=dict)
    provinces: dict[int, Eu4Province] = field(default_factory=dict)


def allocate_tags(uids: list[str], reserved: set[str], prefix: str = "Z") -> dict[str, str]:
    """Deterministic tag allocation that avoids vanilla/reserved tags: Z00, Z01, … Z99, ZA0 …"""
    alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    out: dict[str, str] = {}
    gen = (f"{prefix}{a}{b}" for a in alphabet for b in alphabet)
    for uid in sorted(uids):
        for tag in gen:
            if tag not in reserved:
                out[uid] = tag
                break
        else:
            raise ValueError("ran out of tags")
    return out
