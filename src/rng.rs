//! A seeded stream, so a divergence can be replayed.
//!
//! `SplitMix64`. Small enough to read, good enough to draw missions with, and
//! its own dependency-free so the graded path keeps none.

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// A seed nobody chose, for the run whose job is to explore rather than
    /// to reproduce. It prints itself, so a failure is replayable.
    pub fn seed_from_the_clock() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x2026_0922, |since| {
                since
                    .as_secs()
                    .wrapping_mul(1_000_000_000)
                    .wrapping_add(u64::from(since.subsec_nanos()))
            })
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform over `0..bound`. Zero when the bound is zero, so a caller need
    /// not special-case an empty range.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        let drawn = self.next() % u64::from(bound);
        u32::try_from(drawn).expect("a value below a u32 bound fits in a u32")
    }

    pub fn chance(&mut self, in_this_many: u32) -> bool {
        self.below(in_this_many) == 0
    }

    pub fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        let length = u32::try_from(from.len()).unwrap_or(u32::MAX);
        &from[self.below(length) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn a_seed_names_the_same_stream_every_time() {
        let draw = |seed| {
            let mut rng = Rng::from_seed(seed);
            (0..50).map(|_| rng.below(1000)).collect::<Vec<_>>()
        };
        assert_eq!(draw(1), draw(1));
        assert_ne!(draw(1), draw(2));
    }

    #[test]
    fn below_stays_inside_its_bound() {
        let mut rng = Rng::from_seed(3);
        for bound in [1, 2, 3, 7, 50] {
            for _ in 0..200 {
                assert!(rng.below(bound) < bound);
            }
        }
        assert_eq!(rng.below(0), 0);
    }

    #[test]
    fn the_stream_is_not_obviously_degenerate() {
        let mut rng = Rng::from_seed(11);
        let drawn: std::collections::BTreeSet<u32> = (0..200).map(|_| rng.below(100)).collect();
        assert!(drawn.len() > 50, "only {} distinct values", drawn.len());
    }
}
