//! Reproducibility: every random decision draws from its own named stream.
//! SplitMix64 streams keyed by (seed, stream name). Bit-identical to
//! `tools/research/mega_converter/rng.py` — see the shared test vectors below.

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

pub fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(FNV_OFFSET, |h, &b| (h ^ b as u64).wrapping_mul(FNV_PRIME))
}

pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Never use a global RNG: every decision gets its own stream, e.g.
/// `DetRng::for_stream(seed, "conversion/split/country:000042")`.
#[derive(Debug, Clone)]
pub struct DetRng {
    state: u64,
}

impl DetRng {
    pub fn for_stream(seed: u64, stream: &str) -> Self {
        Self { state: mix64(seed ^ fnv1a64(stream.as_bytes())) }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN);
        mix64(self.state)
    }

    /// Uniform in [0, 1), 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    /// Same vectors as tools/research/tests/test_rng.py — keep them in sync.
    #[test]
    fn cross_language_vectors() {
        let mut r = DetRng::for_stream(45_819_283, "conversion/country:000042");
        assert_eq!(r.next_u64(), 0x928a35586e737730);
        assert_eq!(r.next_u64(), 0x02b32030f4dcd7db);
        assert_eq!(r.next_u64(), 0x7109cb2d0b9a6905);
        assert_eq!(r.next_f64(), 0.09417269202389345);
    }
}
