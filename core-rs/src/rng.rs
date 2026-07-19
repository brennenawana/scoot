//! SplitMix64 + the owned bounded draw (CONTRACTS.md §1–2).

/// Deterministic, seedable PRNG. Rolls must be reproducible from a seed on
/// every platform; not cryptographic — it's a toy capsule machine, and an
/// honest one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Any raw-u64 generator usable by the roll engine.
pub trait RandomSource {
    fn next_u64(&mut self) -> u64;
}

impl RandomSource for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

/// Rejection-modulo bounded uniform draw (CONTRACTS.md §2): unbiased, owned
/// by Scoot so no contract depends on a language stdlib's internals.
pub fn uniform<R: RandomSource>(bound: usize, rng: &mut R) -> usize {
    assert!(bound > 0, "bound must be positive");
    let b = bound as u64;
    let k = (0u64.wrapping_sub(b)) % b; // 2^64 mod b
    loop {
        let r = rng.next_u64();
        if k == 0 || r <= u64::MAX - k {
            return (r % b) as usize;
        }
    }
}
