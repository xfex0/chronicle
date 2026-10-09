import pytest

from mega_converter.rules import EntityContext, RuleEngine, RuleSyntaxError, load_rules, parse_rules
from mega_converter.rules.engine import apply_effects
from mega_converter.rules.history_correction import compile_history_correction


def ctx(uid="country:1", features=None, attrs=None, flags=()):
    return EntityContext(uid, "country", features or {}, attrs or {}, frozenset(flags))


def test_short_form_from_spec():
    rules = parse_rules({"rules": [{
        "name": "ancient_dynasty",
        "if": {"dynasty_age_years": {"greater_than": 300}},
        "then": {"add_modifier": {"legitimacy": 0.10, "prestige": 0.10}},
    }]})
    out = RuleEngine(rules).evaluate([ctx(features={"dynasty_age_years": 450})], "pre_conversion")
    assert out.effects["country:1"].modifiers == {"legitimacy": 0.10, "prestige": 0.10}


def test_combinators_and_dotted_paths():
    rules = parse_rules([{
        "name": "r", "phase": "history",
        "if": {"any": [
            {"economy.trade_dependency": {"greater_than": 0.6}},
            {"all": [{"civil_wars": {"at_least": 3}}, {"not": {"has_flag": "stable"}}]},
        ]},
        "then": [{"add_flag": "hit"}],
    }])
    eng = RuleEngine(rules)
    hits = eng.evaluate([
        ctx("c:1", attrs={"economy": {"trade_dependency": 0.7}}),
        ctx("c:2", features={"civil_wars": 3}),
        ctx("c:3", features={"civil_wars": 3}, flags={"stable"}),
        ctx("c:4", features={"civil_wars": 1}),
    ], "history")
    assert sorted(hits.effects) == ["c:1", "c:2"]


def test_missing_metric_is_false_and_traced():
    rules = parse_rules([{"name": "r", "if": {"nope": {"greater_than": 1}}, "then": {"add_flag": "x"}}])
    out = RuleEngine(rules).evaluate([ctx()], "pre_conversion")
    assert out.effects == {}
    assert out.trace[0].missing_metrics == ("nope",)


def test_priority_order_and_set_value_conflict():
    rules = parse_rules([
        {"name": "low", "priority": 10, "then": {"set_value": {"government": "republic"}}},
        {"name": "high", "priority": 500, "then": {"set_value": {"government": "monarchy"}}},
    ])
    fx = RuleEngine(rules).evaluate([ctx()], "pre_conversion").effects["country:1"]
    assert fx.sets["government"] == ("monarchy", "high")
    assert any("ignored" in n for n in fx.notes)


def test_recommendation_resolution():
    rules = parse_rules([
        {"name": "a", "priority": 100, "then": {"recommend_conversion": "federation"}},
        {"name": "b", "priority": 300, "then": {"recommend_conversion": {"convert_as": "successor_states"}}},
    ])
    out = RuleEngine(rules).evaluate([ctx()], "pre_conversion")
    assert out.best_recommendation("country:1") == ("successor_states", "b")


def test_apply_effects_clamps_and_merges_flags():
    rules = parse_rules([{"name": "r", "then": [
        {"add_modifier": {"stability": 0.5}}, {"add_flag": ["b", "a"]}]}])
    fx = RuleEngine(rules).evaluate([ctx()], "pre_conversion").effects["country:1"]
    out = apply_effects({"stability": 0.8, "flags": ["z"]}, fx)
    assert out["stability"] == 1.0
    assert out["flags"] == ["a", "b", "z"]


@pytest.mark.parametrize("bad, msg", [
    ({"name": "r", "if": {"x": {"bigger": 1}}, "then": {"add_flag": "f"}}, "unknown comparator"),
    ({"name": "r", "then": {"explode": {}}}, "unknown action"),
    ({"name": "r", "scope": "galaxy", "then": {"add_flag": "f"}}, "scope"),
    ({"name": "r", "then": {"add_modifier": {"x": "lots"}}}, "numeric"),
    ({"name": "r", "then": {"recommend_conversion": "anarchy"}}, "convert_as"),
    ({"name": "r", "then": {"protect": {"everything": True}}}, "unknown protect"),
    ({"name": "r", "iff": {}, "then": {"add_flag": "f"}}, "unknown rule keys"),
    ({"name": "r"}, "needs 'then'"),
])
def test_syntax_errors_have_locations(bad, msg):
    with pytest.raises(RuleSyntaxError, match=msg) as e:
        parse_rules([bad], source="test.yaml")
    assert "test.yaml" in str(e.value)


def test_duplicate_names_rejected():
    with pytest.raises(RuleSyntaxError, match="duplicate"):
        parse_rules([{"name": "r", "then": {"add_flag": "a"}}, {"name": "r", "then": {"add_flag": "b"}}])


def test_core_rules_file_is_valid(repo):
    rules = load_rules([repo / "config/rules/core_history.yaml"])
    assert {r.name for r in rules} >= {"ancient_dynasty", "fragmented_state", "merchant_tradition"}


def test_history_correction_protects_and_stricter_limit_wins():
    hc = compile_history_correction({"protect_states": ["country:1"], "limit_assimilation": 0.2})
    extra = parse_rules([{"name": "plugin_limit", "then": {"protect": {"limit_assimilation": 0.1}}}])
    out = RuleEngine(hc + extra).evaluate([ctx("country:1"), ctx("country:2")], "pre_conversion")
    assert out.effects["country:1"].protections == {"state": True, "limit_assimilation": 0.1}
    assert "state" not in out.effects["country:2"].protections
