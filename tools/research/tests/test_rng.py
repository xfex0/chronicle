from mega_converter.rng import DeterministicRng, fnv1a64


def test_fnv_reference():
    assert fnv1a64(b"") == 0xCBF29CE484222325
    assert fnv1a64(b"a") == 0xAF63DC4C8601EC8C


def test_cross_language_vectors():
    """Same vectors as rust/crates/conversion-core/src/rng.rs — keep in sync."""
    r = DeterministicRng(45819283, "conversion/country:000042")
    assert r.next_u64() == 0x928A35586E737730
    assert r.next_u64() == 0x02B32030F4DCD7DB
    assert r.next_u64() == 0x7109CB2D0B9A6905
    assert r.next_f64() == 0.09417269202389345


def test_streams_are_independent_and_reproducible():
    a = [DeterministicRng(1, "s/a").next_f64() for _ in range(2)]
    assert a[0] == a[1]
    assert DeterministicRng(1, "s/a").next_u64() != DeterministicRng(1, "s/b").next_u64()
