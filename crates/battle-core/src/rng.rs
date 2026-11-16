/// Versioned SplitMix64 stream. All draws go through the same session-local state.
pub(crate) struct Rng {
    state: u64,
    pub draws: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed.into(),
            draws: 0,
        }
    }

    fn next(&mut self) -> u64 {
        self.draws += 1;
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// Sample a known nonempty range using multiply-high with rejection of biased draws.
    pub fn below(&mut self, bound: u64) -> u64 {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let product = u128::from(self.next()) * u128::from(bound);
            if product as u64 >= threshold {
                return (product >> 64) as u64;
            }
        }
    }
}
