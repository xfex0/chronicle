from mega_converter.conversion.state_transform import StructureInputs, TransformConfig, decide

FRAGMENTED = StructureInputs(centralization=0.20, vassal_autonomy=0.80, civil_wars=7,
                             culture_fragmentation=0.70, legitimacy=0.35)
CENTRALIZED = StructureInputs(centralization=0.90, vassal_autonomy=0.10, civil_wars=0,
                              culture_fragmentation=0.10, legitimacy=0.90)


def test_spec_examples():
    cfg = TransformConfig()
    assert decide("a", FRAGMENTED, cfg).convert_as == "successor_states"
    assert decide("b", CENTRALIZED, cfg).convert_as == "unified_state"


def test_protection_blocks_collapse():
    d = decide("a", FRAGMENTED, TransformConfig(), protections={"prevent_collapse": True})
    assert (d.convert_as, d.source) == ("federation", "protection")
    d = decide("a", FRAGMENTED, TransformConfig(allow_state_collapse=False))
    assert d.convert_as == "federation"


def test_override_beats_everything():
    d = decide("a", FRAGMENTED, TransformConfig(), override="unified_state",
               rule_recommendation=("successor_states", "r"))
    assert (d.convert_as, d.source) == ("unified_state", "override")


def test_divergence_moves_thresholds():
    strict = TransformConfig(historical_divergence=0.0).effective_thresholds()
    wild = TransformConfig(historical_divergence=1.0).effective_thresholds()
    assert strict[1] > wild[1]
