import copy
import json

import yaml

from mega_converter.config import load_campaign
from mega_converter.pipeline.context import PipelineContext
from mega_converter.pipeline.runner import run
from mega_converter.rules import load_rules
from mega_converter.rules.history_correction import compile_history_correction


def _ctx(repo, overrides=None):
    cfg = load_campaign(repo / "config/campaign.example.yaml")
    world = json.loads((repo / "tests/fixtures/synthetic/world_two_states.json").read_text())
    events = yaml.safe_load((repo / "tests/fixtures/synthetic/events_two_states.yaml").read_text())
    rules = load_rules([repo / "config" / p for p in cfg.rule_files]) + compile_history_correction(cfg.history_correction)
    return PipelineContext(cfg, world, "ck3", "eu4", events=events, rules=rules, overrides=overrides or {})


def test_preview_on_synthetic_world(repo):
    ctx = run(_ctx(repo))
    assert ctx.decisions["country:000001"].convert_as == "successor_states"
    assert ctx.decisions["country:000002"].convert_as == "unified_state"
    assert "country:000003" not in ctx.decisions  # subjects follow overlord
    assert "ancient_dynasty_tradition" in ctx.world["countries"]["country:000002"]["flags"]


def test_user_override_applied(repo):
    ctx = run(_ctx(repo, {"country:000001": {"convert_as": "unified_state"}}))
    assert ctx.decisions["country:000001"].source == "override"


def test_deterministic(repo):
    a, b = run(_ctx(repo)), run(_ctx(repo))
    assert a.decisions == b.decisions
    assert json.dumps(a.world, sort_keys=True) == json.dumps(copy.deepcopy(b.world), sort_keys=True)
