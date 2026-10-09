from mega_converter.simulation.history_features import compute_features, years_between

U = "country:1"


def ev(y, kind, payload=None, actors=(U,)):
    return {"date": [y, 1, 1], "kind": kind, "actors": list(actors), "payload": payload or {}}


def test_counts_only_past_events_of_entity():
    events = [ev(1100, "civil_war"), ev(1200, "civil_war"), ev(1600, "civil_war"),
              ev(1150, "civil_war", actors=("country:2",))]
    f = compute_features(U, (1453, 1, 1), {}, events)
    assert f["civil_wars"] == 2


def test_dynasty_age_tracks_current_dynasty():
    events = [ev(844, "state_created", {"dynasty": "d:1"}),
              ev(1000, "dynasty_change", {"to": "d:2"}),
              ev(1100, "dynasty_change", {"to": "d:1"})]
    assert compute_features(U, (1453, 1, 1), {"ruling_dynasty": "d:1"}, events)["dynasty_age_years"] == 353
    assert compute_features(U, (1050, 1, 1), {"ruling_dynasty": "d:2"}, events)["dynasty_age_years"] == 50


def test_years_by_government():
    events = [ev(1000, "government_changed", {"to": "republic"}),
              ev(1400, "government_changed", {"to": "monarchy"})]
    f = compute_features(U, (1450, 1, 1), {}, events)
    assert f["years_by_government"] == {"republic": 400, "monarchy": 50}


def test_bc_dates():
    assert years_between((-4, 1, 1), (-304, 1, 1)) == 300
    assert years_between((1, 6, 1), (-1, 7, 1)) == 1
