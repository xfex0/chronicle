"""Shared expectations with rust/crates/chronicle-bridge (same seed ⇒ same world)."""

from mega_converter.era_bridge import PRESETS, load, simulate

SEED = 45819283


def run(name):
    return simulate(PRESETS[name], SEED, load("config/bridge.yaml"))


def test_spec_example_global_democracy():
    d = run("global_democracy").design
    assert d["authority"] == "democratic"
    assert sorted(d["ethics"]) == ["egalitarian", "materialist", "xenophile"]
    assert d["civics"] == ["beacon_of_liberty", "technocracy"]
    assert d["origin"] == "prosperous_unification"


def test_spec_example_military_dictatorship():
    r = run("military_dictatorship")
    assert r.design["authority"] == "dictatorial"
    assert sorted(r.design["ethics"]) == ["authoritarian", "militarist", "xenophobe"]
    assert r.design["civics"] == ["distinguished_admiralty", "nationalistic_zeal"]
    assert ("1968", "conquest") == (str(r.events[0][0]), r.events[0][1])


def test_planned_technocracy_becomes_mechanists():
    d = run("planned_technocracy").design
    assert d["origin"] == "mechanists"
    assert d["ethics"][0] == "fanatic_materialist"


def test_deterministic():
    a, b = run("cold_war"), run("cold_war")
    assert a.events == b.events and a.design == b.design


def test_always_three_ethic_points():
    cfg = load("config/bridge.yaml")
    for name, p in PRESETS.items():
        for seed in range(40):
            e = simulate(p, seed, cfg).design["ethics"]
            assert sum(2 if x.startswith("fanatic_") else 1 for x in e) == 3, (name, seed, e)
