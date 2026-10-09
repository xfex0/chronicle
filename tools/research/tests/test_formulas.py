import pytest

from mega_converter.conversion.formulas import FormulaError, SafeExpr, load_formulas, merge_formulas, parse_formulas


def test_long_and_short_forms():
    fs = parse_formulas({
        "ck3_to_eu4": {
            "province_tax": {"normalized": True, "scale": 10,
                             "inputs": {"development": 0.4, "control": 0.2, "prosperity": 0.2, "urbanization": 0.2}},
            "manpower": {"population": 0.6, "development": 0.2, "military_tradition": 0.2},
        }
    })
    tax = fs["ck3_to_eu4"]["province_tax"]
    assert tax({"development": 1, "control": 0.5, "prosperity": 0, "urbanization": 0.5}) == pytest.approx(6.0)
    assert fs["ck3_to_eu4"]["manpower"].inputs == {"population", "development", "military_tradition"}


def test_unnormalized_weights_rejected():
    with pytest.raises(FormulaError, match="sum to"):
        parse_formulas({"t": {"f": {"a": 0.5, "b": 0.6}}})


def test_missing_input():
    fs = parse_formulas({"t": {"f": {"a": 1.0}}})
    with pytest.raises(FormulaError, match="missing input 'a'"):
        fs["t"]["f"]({})


def test_safe_expression():
    e = SafeExpr("clamp(base * (1 + stability), 1, 30)")
    assert e.variables == {"base", "stability"}
    assert e({"base": 10, "stability": 0.5}) == 15
    assert e({"base": 100, "stability": 0.5}) == 30


@pytest.mark.parametrize("src", [
    "__import__('os').system('x')",
    "a.__class__",
    "[x for x in y]",
    "lambda: 1",
    "a if b else c",
    "open('f')",
    "2 ** 1000000",
])
def test_unsafe_expressions_rejected(src):
    with pytest.raises(FormulaError):
        SafeExpr(src)


def test_overlay_merges_by_name():
    base = parse_formulas({"t": {"f": {"a": 1.0}, "g": {"b": 1.0}}})
    over = parse_formulas({"t": {"f": {"c": 1.0}}})
    m = merge_formulas(base, over)
    assert m["t"]["f"].inputs == {"c"} and m["t"]["g"].inputs == {"b"}


def test_repo_formulas_valid(repo):
    fs = load_formulas(repo / "config/formulas/conversion_formulas.yaml")
    assert "ck3_to_eu4" in fs
