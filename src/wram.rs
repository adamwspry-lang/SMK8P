pub const RACER_STRUCT_BASE: usize = 0x1000;

pub const RACER_STRUCT_SIZE: usize = 0x100;

pub const WRAM_SIZE: usize = 0x2_0000;

pub const NUM_KART_SLOTS: u8 = 8;

pub const RNG_ADDR: usize = 0x1F26;

pub const LOCAL_KART_SLOT: u8 = 0;

pub const GAMEMODE_ADDR: usize = 0x0036;

pub const GAMEMODE_IN_RACE: u8 = 0x02;

pub const GAMEMODE_TITLE: u8 = 0x04;

pub const GAMEMODE_TRACK_SELECT: u8 = 0x08;

pub const GAMEMODE_NONE: u8 = 0x00;

pub const CUP_NUMBER_ADDR: usize = 0x0150;

pub const RACE_NUMBER_ADDR: usize = 0x0152;

pub const NUM_TRACKS: u8 = 20;

pub const CURRENT_LAP_OFFSET: usize = 0xC1;

pub const LAP_NOT_STARTED_VALUE: u8 = 0x7F;

pub const CURRENT_RANK_OFFSET: usize = 0x40;

pub const RACE_STARTED_LAP_VALUE: u8 = 0x80;

pub const LAKITU_SCRIPT_PTR_ADDR: usize = 0x1C04;

pub const LAKITU_PARKED_VALUE: u16 = 0xDC06;

#[allow(dead_code)]
pub const LAKITU_RELEASED_VALUE: u16 = 0xDC14;

pub const Z_POSITION_OFFSET: usize = 0x20;
pub const Z_POSITION_MAX: u16 = 0xFFFF;

pub const FINISHED_LAP_VALUE: u8 = 0x85;

pub const COUNTDOWN_HOLD_ADDR: usize = 0x0E5A;

pub const READY_SIGNAL_ADDR: usize = 0x1C30;

#[allow(dead_code)]
pub const READY_SIGNAL_VALUE: u8 = 0x02;

pub fn racer_offset(kart_slot: u8) -> usize {
    debug_assert!(kart_slot < NUM_KART_SLOTS, "kart_slot out of range: {kart_slot}");
    let offset = RACER_STRUCT_BASE + (kart_slot as usize) * RACER_STRUCT_SIZE;
    debug_assert!(
        offset + RACER_STRUCT_SIZE <= WRAM_SIZE,
        "racer struct for kart_slot {kart_slot} falls outside WRAM (offset {offset:#x})"
    );
    offset
}

pub const MAILBOX_ADDR: usize = 0x0E58;

pub const CAN_ENTER_RACE_BIT: u8 = 0b0000_0001;

pub const ALL_READY_BIT: u8 = 0b0000_0010;

pub type ServerSlot = u8;

pub type KartSlot = u8;

#[derive(Debug, Clone)]
pub struct SlotMap {
    assigned: std::collections::HashMap<ServerSlot, KartSlot>,
    
    free_pool: Vec<KartSlot>,
}

impl SlotMap {
    pub fn new() -> Self {
        let mut free_pool: Vec<KartSlot> = (1..NUM_KART_SLOTS).collect();
        
        free_pool.reverse();
        Self {
            assigned: std::collections::HashMap::new(),
            free_pool,
        }
    }

    pub fn assign(&mut self, server_slot: ServerSlot) -> Option<KartSlot> {
        if let Some(&kart_slot) = self.assigned.get(&server_slot) {
            return Some(kart_slot);
        }
        let kart_slot = self.free_pool.pop()?;
        self.assigned.insert(server_slot, kart_slot);
        Some(kart_slot)
    }

    pub fn get(&self, server_slot: ServerSlot) -> Option<KartSlot> {
        self.assigned.get(&server_slot).copied()
    }

    pub fn free(&mut self, server_slot: ServerSlot) {
        if let Some(kart_slot) = self.assigned.remove(&server_slot) {
            self.free_pool.push(kart_slot);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ServerSlot, KartSlot)> + '_ {
        self.assigned.iter().map(|(&server_slot, &kart_slot)| (server_slot, kart_slot))
    }
}

impl Default for SlotMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn racer_offset_matches_ram_map() {
        
        assert_eq!(racer_offset(0), 0x1000);
        assert_eq!(racer_offset(1), 0x1100);
        assert_eq!(racer_offset(7), 0x1700);
    }

    #[test]
    fn all_racer_offsets_fit_inside_wram() {
        for kart_slot in 0..NUM_KART_SLOTS {
            let offset = racer_offset(kart_slot);
            assert!(
                offset + RACER_STRUCT_SIZE <= WRAM_SIZE,
                "kart_slot {kart_slot} offset {offset:#x} + struct size overflows WRAM_SIZE {WRAM_SIZE:#x}"
            );
        }
    }

    #[test]
    fn mailbox_addr_fits_inside_wram() {
        assert!(MAILBOX_ADDR < WRAM_SIZE);
    }

    #[test]
    fn rng_addr_fits_inside_wram() {
        assert!(RNG_ADDR + 1 < WRAM_SIZE);
    }

    #[test]
    fn assigns_lowest_free_slot_first() {
        let mut map = SlotMap::new();
        assert_eq!(map.assign(3), Some(1));
        assert_eq!(map.assign(5), Some(2));
        assert_eq!(map.assign(7), Some(3));
    }

    #[test]
    fn same_server_slot_always_returns_same_kart_slot() {
        let mut map = SlotMap::new();
        let first = map.assign(3).unwrap();
        for _ in 0..5 {
            assert_eq!(map.assign(3), Some(first));
        }
    }

    #[test]
    fn freeing_reclaims_the_slot() {
        let mut map = SlotMap::new();
        map.assign(3);
        map.assign(5);
        map.free(3);
        
        assert_eq!(map.assign(7), Some(1));
    }

    #[test]
    fn get_does_not_assign() {
        let map = SlotMap::new();
        assert_eq!(map.get(3), None);
    }

    #[test]
    fn iter_yields_all_assigned_pairs() {
        let mut map = SlotMap::new();
        map.assign(3);
        map.assign(5);
        let mut pairs: Vec<_> = map.iter().collect();
        pairs.sort();
        assert_eq!(pairs, vec![(3, 1), (5, 2)]);
    }

    #[test]
    fn free_pool_never_hands_out_kart_slot_zero() {
        let mut map = SlotMap::new();
        for server_slot in 0..7 {
            let kart_slot = map.assign(server_slot).unwrap();
            assert_ne!(kart_slot, LOCAL_KART_SLOT);
        }
    }

    #[test]
    fn exhausting_the_pool_returns_none() {
        let mut map = SlotMap::new();
        for server_slot in 0..7 {
            assert!(map.assign(server_slot).is_some());
        }
        
        assert_eq!(map.assign(7), None);
    }

    #[test]
    fn mailbox_bits_are_distinct() {
        assert_eq!(CAN_ENTER_RACE_BIT & ALL_READY_BIT, 0);
    }

    #[test]
    fn gamemode_values_are_distinct() {
        
        let values = [GAMEMODE_NONE, GAMEMODE_IN_RACE, GAMEMODE_TITLE, GAMEMODE_TRACK_SELECT];
        for (i, a) in values.iter().enumerate() {
            for (j, b) in values.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "gamemode constants must be pairwise distinct");
                }
            }
        }
    }

    #[test]
    fn gamemode_addr_fits_inside_wram() {
        assert!(GAMEMODE_ADDR < WRAM_SIZE);
    }

    #[test]
    fn pre_lakitu_offset_matches_local_racer_struct() {
        
        assert_eq!(racer_offset(LOCAL_KART_SLOT) + CURRENT_LAP_OFFSET, 0x10C1);
    }

    #[test]
    fn lakitu_script_ptr_addr_fits_inside_wram() {
        assert!(LAKITU_SCRIPT_PTR_ADDR + 1 < WRAM_SIZE);
    }

    #[test]
    fn lakitu_parked_and_released_values_are_distinct() {
        assert_ne!(LAKITU_PARKED_VALUE, LAKITU_RELEASED_VALUE);
    }

    #[test]
    fn lap_not_started_value_is_distinct_from_other_lap_values() {
        assert_ne!(LAP_NOT_STARTED_VALUE, RACE_STARTED_LAP_VALUE);
        assert_ne!(LAP_NOT_STARTED_VALUE, FINISHED_LAP_VALUE);
    }
}