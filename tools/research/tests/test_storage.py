import os
import stat

from mega_converter.ai.advisor import CachedAdvisor, Recommendation
from mega_converter.conversion.overrides import Override, OverrideStore
from mega_converter.database import CampaignStore
from mega_converter.pipeline.workspace import Workspace


def test_campaign_db_roundtrip(tmp_path):
    db = CampaignStore(tmp_path / "campaign.db")
    assert db.schema_version == 1
    cid = db.create_campaign("test", 42, "campaign: {}", "0.1.0")
    sid = db.add_snapshot(cid, game="ck3", kind="import", world_path="snapshots/1.json.zst",
                          world_sha256="00", uwm_schema_version=1, date=(867, 1, 1))
    db.add_events(cid, [{"date": [-304, 1, 1], "kind": "state_created", "actors": ["country:1"]}], "save", sid)
    ev = db.events(cid)
    assert ev[0]["date"] == [-304, 1, 1] and ev[0]["actors"] == ["country:1"]
    assert db.snapshots(cid)[0]["game"] == "ck3"
    db.close()


class FakeAdvisor:
    model = "fake"

    def __init__(self):
        self.calls = 0

    def advise(self, task, entity_uid, payload):
        self.calls += 1
        return Recommendation(task, entity_uid, {"government": "constitutional_monarchy"}, 0.82, "x", self.model)


def test_ai_recommendations_are_cached(tmp_path):
    db = CampaignStore(tmp_path / "c.db")
    cid = db.create_campaign("t", 1, "", "0.1.0")
    inner = FakeAdvisor()
    adv = CachedAdvisor(inner, db, cid)
    a = adv.advise("recommend_government", "country:1", {"centralization": 0.8})
    b = adv.advise("recommend_government", "country:1", {"centralization": 0.8})
    assert inner.calls == 1 and a == b


def test_import_save_never_touches_original(tmp_path):
    original = tmp_path / "autosave.ck3"
    original.write_bytes(b"SAVtest\nmeta={}\n")
    before = original.stat()
    ws = Workspace.create(tmp_path / "camp")
    imp = ws.import_save(original)
    assert imp.input_copy.read_bytes() == original.read_bytes()
    assert not (os.stat(imp.input_copy).st_mode & stat.S_IWUSR)
    assert original.stat().st_mtime == before.st_mtime
    assert ws.import_save(original).backup == imp.backup  # idempotent, content-addressed


def test_overrides_persist(tmp_path):
    p = tmp_path / "campaign_overrides.yaml"
    s = OverrideStore(p)
    s.set(Override("country:1", "convert_as", "unified_state"))
    assert OverrideStore(p).get("country:1", "convert_as") == "unified_state"
    s.remove("country:1", "convert_as")
    assert OverrideStore(p).all() == []
