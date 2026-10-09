"""Deterministic, cross-language RNG.

Bit-for-bit identical to ``rust/crates/conversion-core/src/rng.rs`` so that a Python
stage and a Rust stage drawing from the same stream produce the same numbers.

Every random decision draws from its own *stream*, derived from the campaign seed and a
stable name such as ``"conversion/split/country:000042"``. This keeps results identical
regardless of iteration order or parallelism.
"""

from __future__ import annotations

MASK64 = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15
FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3


def fnv1a64(data: bytes) -> int:
    h = FNV_OFFSET
    for b in data:
        h ^= b
        h = (h * FNV_PRIME) & MASK64
    return h


def mix64(z: int) -> int:
    z &= MASK64
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    return z ^ (z >> 31)


class DeterministicRng:
    """SplitMix64 stream keyed by (seed, stream name)."""

    __slots__ = ("_state",)

    def __init__(self, seed: int, stream: str) -> None:
        self._state = mix64((seed & MASK64) ^ fnv1a64(stream.encode("utf-8")))

    def next_u64(self) -> int:
        self._state = (self._state + GOLDEN) & MASK64
        return mix64(self._state)

    def next_f64(self) -> float:
        """Uniform in [0, 1) with 53 bits of precision."""
        return (self.next_u64() >> 11) * (1.0 / (1 << 53))

    def chance(self, p: float) -> bool:
        return self.next_f64() < p
