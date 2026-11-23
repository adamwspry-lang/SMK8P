use crate::rng::lfsr_table::LFSR_TABLE;
use std::sync::OnceLock;

const DEAD: u16 = u16::MAX;

fn reverse_index() -> &'static [u16; 65536] {
    static REVERSE: OnceLock<[u16; 65536]> = OnceLock::new();
    REVERSE.get_or_init(|| {
        let mut table = [DEAD; 65536];
        for (idx, &value) in LFSR_TABLE.iter().enumerate() {
            debug_assert_eq!(
                table[value as usize], DEAD,
                "LFSR_TABLE contains a duplicate value {value:#06x} at index {idx} -- table is corrupt"
            );
            table[value as usize] = idx as u16;
        }
        table
    })
}

pub struct HazardRng {
    idx: usize,
}

impl HazardRng {
    pub fn from_seed(seed: u16) -> Self {
        let idx = reverse_index()[seed as usize];
        if idx == DEAD {
            eprintln!(
                "HazardRng::from_seed: seed {seed:#06x} is one of the two dead LFSR states \
                 (0xAA55/0x54AA) and never appears in the real cycle -- falling back to index 0. \
                 This should be prevented server-side (don't generate these two values as a seed)."
            );
            HazardRng { idx: 0 }
        } else {
            HazardRng { idx: idx as usize }
        }
    }

    pub fn next(&mut self) -> u16 {
        self.idx = (self.idx + 1) % LFSR_TABLE.len();
        LFSR_TABLE[self.idx]
    }
}

impl crate::rng::fish::RngSource for HazardRng {
    fn next(&mut self) -> u16 {
        HazardRng::next(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_value_round_trips_through_the_reverse_index() {
        for (idx, &value) in LFSR_TABLE.iter().enumerate() {
            assert_eq!(reverse_index()[value as usize] as usize, idx);
        }
    }

    #[test]
    fn dead_states_are_absent_and_flagged() {
        assert_eq!(reverse_index()[0xAA55], DEAD);
        assert_eq!(reverse_index()[0x54AA], DEAD);
    }

    #[test]
    fn from_seed_zero_starts_at_the_table_head() {
        let mut rng = HazardRng::from_seed(0x0000);
        assert_eq!(rng.next(), LFSR_TABLE[1]);
    }
}