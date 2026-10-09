"""Modern Era Bridge — research prototype.

Same model as rust/crates/chronicle-bridge (the shipped implementation); used to calibrate
config/bridge.yaml quickly. Shared expected outputs are asserted on both sides.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

from mega_converter.rng import DeterministicRng

ROOT = Path(__file__).resolve().parents[3]
SOCIAL = ("authoritarianism", "social_equality", "xenophobia", "international_cooperation",
          "religious_influence", "militarism", "economic_planning", "environmental_policy")
AXES = [("authoritarian_egalitarian", "authoritarian", "egalitarian"),
        ("xenophobe_xenophile", "xenophobe", "xenophile"),
        ("militarist_pacifist", "militarist", "pacifist"),
        ("spiritualist_materialist", "spiritualist", "materialist")]


def load(name: str) -> dict[str, Any]:
    return yaml.safe_load((ROOT / name).read_text(encoding="utf-8"))


@dataclass
class Civ:
    values: dict[str, float]
    blocs: int = 2
    ideology: str = "democracy"

    def __getitem__(self, k: str) -> float:
        return self.values[k]

    def add(self, k: str, dv: float) -> None:
        self.values[k] = min(1.0, max(0.0, self.values[k] + dv))


@dataclass
class Result:
    final: Civ
    events: list[tuple[int, str]] = field(default_factory=list)
    flags: set[str] = field(default_factory=set)
    design: dict[str, Any] = field(default_factory=dict)


def weighted(w: dict[str, float], c: Civ) -> float:
    # Plain left-to-right accumulation (not sum(): Python 3.12+ uses compensated summation),
    # so results are bit-identical to the Rust fold.
    acc = 0.0
    for k, v in sorted(w.items()):
        acc = acc + v * (1.0 if k == "constant" else c[k])
    return acc


def simulate(start: Civ, seed: int, cfg: dict[str, Any]) -> Result:
    c = Civ(dict(start.values), start.blocs, start.ideology)
    d, ev = cfg["drift"], cfg["events"]
    res = Result(final=c)
    year, step = cfg["start_year"], cfg["step_years"]
    reached: set[str] = set()
    unified = False
    while year < cfg["end_year"]:
        rng = DeterministicRng(seed, f"bridge/{year}")
        c.add("technology", d["technology"] * (1 - c["technology"]))
        c.add("industrialization", d["industrialization"] * (1 - c["industrialization"]))
        boost = 1 + (d["space_race_bonus"] if c.blocs > 1 else 0) + d["space_cooperation_bonus"] * c["international_cooperation"]
        c.add("space_program", d["space_from_technology"] * c["technology"] * boost * (1 - c["space_program"]))
        for k in SOCIAL:
            c.add(k, d["mean_reversion"] * (0.5 - c[k]) + (rng.next_f64() * 2 - 1) * d["noise"])
        target = c["militarism"] * (1 - c["international_cooperation"]) * d["wars_target_militarism"]
        c.add("global_wars", d["wars_relaxation"] * (target - c["global_wars"]))
        c.add("planetary_unification",
              d["unification_from_cooperation"] * c["international_cooperation"] * (1 - c["planetary_unification"]))

        tension = min(1.0, (c.blocs - 1) / 2) * (0.5 + 0.5 * c["militarism"])
        nw = ev["nuclear_war"]
        if rng.chance(nw["base"] * c["nuclear_weapons"] * (1 - c["international_cooperation"]) * tension):
            for k, v in sorted(nw["effects"].items()):
                c.add(k, v)
            res.events.append((year, "nuclear_war"))
            res.flags.add("nuclear_war")
        cc = ev["climate_crisis"]
        if c["industrialization"] >= cc["min_industrialization"] and rng.chance(
                cc["base"] * c["industrialization"] * (1 - c["environmental_policy"])):
            kind = "ecological_collapse" if c["environmental_policy"] < cc["collapse_below_environment"] else "climate_crisis"
            for k, v in sorted(cc["effects"].items()):
                c.add(k, v)
            res.events.append((year, kind))
            res.flags.add(kind)
        if c.blocs > 1:
            q, m = ev["conquest"], ev["peaceful_merge"]
            if c["militarism"] >= q["min_militarism"] and rng.chance(q["base"] * c["militarism"] * (1 - c["international_cooperation"])):
                c.blocs -= 1
                for k, v in sorted(q["effects"].items()):
                    c.add(k, v)
                res.events.append((year, "conquest"))
                res.flags.add("unified_by_conquest")
            elif c["international_cooperation"] >= m["min_cooperation"] and rng.chance(m["base"] * c["international_cooperation"]):
                c.blocs -= 1
                for k, v in sorted(m["effects"].items()):
                    c.add(k, v)
                res.events.append((year, "blocs_merged"))
        for ms in cfg["space_milestones"]:
            name, thr = ms["name"], ms["at"]
            if name not in reached and c["space_program"] >= thr:
                reached.add(name)
                res.events.append((year, f"space_{name}"))
        if not unified and c["planetary_unification"] >= ev["unified_at"]:
            unified = True
            res.events.append((year, "earth_unified"))
            res.flags.add("earth_unified")
        year += step
    res.design = design(c, res.flags, cfg, load("games/stellaris/vocabulary.yaml"))
    return res


def design(c: Civ, flags: set[str], cfg: dict[str, Any], vocab: dict[str, Any]) -> dict[str, Any]:
    s = cfg["stellaris"]
    scores = {name: max(-1.0, min(1.0, weighted(cfg["axes"][name], c))) for name, _, _ in AXES}
    ranked = sorted(AXES, key=lambda a: (-abs(scores[a[0]]), a[0]))

    def pick(a: tuple[str, str, str]) -> str:
        return a[1] if scores[a[0]] >= 0 else a[2]

    if abs(scores[ranked[0][0]]) >= s["fanatic_threshold"]:
        ethics = ["fanatic_" + pick(ranked[0]), pick(ranked[1])]
    else:
        ethics = [pick(a) for a in ranked[:3]]
    base = {e.removeprefix("fanatic_") for e in ethics}

    a = c["authoritarianism"]
    if c.ideology == "monarchy" and a >= s["imperial_min"]:
        auth = "imperial"
    elif 1 - a >= s["democratic_min"]:
        auth = "democratic"
    elif a >= s["dictatorial_min"]:
        auth = "dictatorial"
    else:
        auth = "oligarchic"
    if base & {f.removeprefix("fanatic_") for f in vocab["authorities"][auth]["forbids_ethics"]}:
        auth = "oligarchic"

    def allowed(key: str) -> bool:
        cv = vocab["civics"][key]
        if cv.get("authorities") and auth not in cv["authorities"]:
            return False
        if any(r not in base for r in cv.get("requires_ethics", [])):
            return False
        return not any(f in base for f in cv.get("forbids_ethics", []))

    civics = sorted(((weighted(w, c), k) for k, w in cfg["civic_scores"].items() if allowed(k)),
                    key=lambda t: (-t[0], t[1]))
    automation = c["technology"] * c["industrialization"] * (0.5 + 0.5 * c["economic_planning"])
    if "nuclear_war" in flags:
        origin = "post_apocalyptic"
    elif "ecological_collapse" in flags:
        origin = "doomsday"
    elif "materialist" in base and automation >= s["mechanists_automation_min"]:
        origin = "mechanists"
    else:
        origin = "prosperous_unification"
    return {"authority": auth, "ethics": ethics, "civics": [k for _, k in civics[:2]], "origin": origin,
            "axes": scores, "civic_scores": civics[:4], "automation": automation}


def civ(blocs: int, ideology: str, **v: float) -> Civ:
    return Civ(dict(v), blocs, ideology)


PRESETS: dict[str, Civ] = {
    "global_democracy": civ(1, "democracy", authoritarianism=0.1, social_equality=0.9, xenophobia=0.1,
                            international_cooperation=0.9, religious_influence=0.2, militarism=0.2, global_wars=0.05,
                            nuclear_weapons=0.4, industrialization=0.7, technology=0.6, space_program=0.1,
                            economic_planning=0.3, environmental_policy=0.6, planetary_unification=0.5),
    "military_dictatorship": civ(3, "fascism", authoritarianism=0.9, social_equality=0.1, xenophobia=0.85,
                                 international_cooperation=0.1, religious_influence=0.4, militarism=0.9, global_wars=0.8,
                                 nuclear_weapons=0.8, industrialization=0.6, technology=0.5, space_program=0.1,
                                 economic_planning=0.6, environmental_policy=0.2, planetary_unification=0.2),
    "cold_war": civ(2, "non_aligned", authoritarianism=0.5, social_equality=0.5, xenophobia=0.5,
                    international_cooperation=0.3, religious_influence=0.4, militarism=0.6, global_wars=0.3,
                    nuclear_weapons=0.9, industrialization=0.6, technology=0.55, space_program=0.15,
                    economic_planning=0.5, environmental_policy=0.3, planetary_unification=0.2),
    "planned_technocracy": civ(2, "communism", authoritarianism=0.45, social_equality=0.75, xenophobia=0.3,
                               international_cooperation=0.7, religious_influence=0.1, militarism=0.3, global_wars=0.1,
                               nuclear_weapons=0.3, industrialization=0.8, technology=0.7, space_program=0.2,
                               economic_planning=0.9, environmental_policy=0.5, planetary_unification=0.4),
}
