pub const NUM_KART_SLOTS: u8 = 8;
pub const LOCAL_KART_SLOT: u8 = 0;

pub const RACER_STRUCT_BASE: usize = 0x1000;
pub const RACER_STRUCT_SIZE: usize = 0x100;
pub const CURRENT_LAP_OFFSET: usize = 0xC1;
pub const LAP_NOT_STARTED_VALUE: u8 = 0x7F;
pub const RACE_STARTED_LAP_VALUE: u8 = 0x80;
pub const CURRENT_LAP_LAP2_VALUE: u8 = 0x81;
pub const FINISHED_LAP_VALUE: u8 = 0x85;
pub const Z_POSITION_OFFSET: usize = 0x20;
pub const Z_POSITION_MAX: u16 = 0xFFFF;
pub const CURRENT_TILE_INDEX_OFFSET: usize = 0x58;
pub const CURRENT_TILE_TYPE_OFFSET: usize = 0x68;

pub const RNG_ADDR: usize = 0x1F26;

pub const GAMEMODE_ADDR: usize = 0x0036;
pub const GAMEMODE_NONE: u8 = 0x00;
pub const GAMEMODE_IN_RACE: u8 = 0x02;
pub const GAMEMODE_TITLE: u8 = 0x04;
pub const GAMEMODE_TRACK_SELECT: u8 = 0x08;
pub const CUP_NUMBER_ADDR: usize = 0x0150;
pub const RACE_NUMBER_ADDR: usize = 0x0152;

pub const LAKITU_SCRIPT_PTR_ADDR: usize = 0x1C04;
pub const LAKITU_PARKED_VALUE: u16 = 0xDC06;
pub const PAUSE_MENU_ADDR: usize = 0x0163;
pub const PAUSE_MENU_OPEN_BIT: u8 = 0x80;
pub const MAILBOX_ADDR: usize = 0x0E58;
pub const CAN_ENTER_RACE_BIT: u8 = 0b0000_0001;
pub const ALL_READY_BIT: u8 = 0b0000_0010;

pub const ITEM_SLOTS_ADDR: usize = 0x1A00;
pub const ITEM_SLOT_SIZE: usize = 0x80;
pub const NUM_ITEM_SLOTS: usize = 2;
pub const ITEM_SLOT_OCCUPIED_OFFSET: usize = 0x12;
pub const ITEM_SLOT_OCCUPIED_BIT: u16 = 0x8000;
pub const ITEM_SLOT_OWNER_OFFSET: usize = 0x6A;
pub const ITEM_SLOT_TYPE_OFFSET: usize = 0x70;
pub const ITEM_SLOT_Z_VELOCITY_OFFSET: usize = 0x26;
pub const ITEM_HIT_Z_VELOCITY_KICK: u16 = 0x0100;
pub const ITEM_SLOT_COLLISION_TIMER_OFFSET: usize = 0x42;
pub const ITEM_HIT_COLLISION_TIMER_VALUE: u16 = 0x8000;
pub const ITEM_HIT_FLAG: u16 = 0x8000;
pub const ITEM_TYPE_BANANA: u8 = 0;
pub const ITEM_TYPE_GREEN_SHELL: u8 = 1;
pub const ITEM_TYPE_RED_SHELL: u8 = 2;
pub const NUM_ITEM_TYPES: u8 = 3;
pub const ITEM_THROW_REQUEST_SLOT_SHIFT: u8 = 2;
pub const ITEM_THROW_REQUEST_SLOT_MASK: u16 = 0x001C;
pub const ITEM_THROW_REQUEST_TYPE_SHIFT: u8 = 5;
pub const ITEM_THROW_REQUEST_TYPE_MASK: u16 = 0x0060;
pub const ITEM_THROW_DIRECTION_BIT: u16 = 0x0800;
pub const ITEM_THROW_REQUEST_BITS_MASK: u16 =
    ITEM_THROW_REQUEST_SLOT_MASK | ITEM_THROW_REQUEST_TYPE_MASK | ITEM_THROW_DIRECTION_BIT;
pub const ITEM_THROW_REQUEST_PRESERVE_MASK: u16 = !ITEM_THROW_REQUEST_BITS_MASK;

pub const LIGHTNING_REQUEST_BIT: u16 = 0x0080;
pub const LIGHTNING_SLOT_SHIFT: u8 = 8;
pub const LIGHTNING_SLOT_MASK: u16 = 0x0700;
pub const LIGHTNING_REQUEST_BITS_MASK: u16 = LIGHTNING_REQUEST_BIT | LIGHTNING_SLOT_MASK;
pub const LIGHTNING_REQUEST_PRESERVE_MASK: u16 = !LIGHTNING_REQUEST_BITS_MASK;
pub const LIGHTNING_SFX_FLAG_ADDR: usize = 0x00D3;

pub const LOADING_SCREEN_BOOL_ADDR: usize = 0x0120;
pub const LOADING_SCREEN_BOOL_LOADING: u16 = 0x0000;

pub const TILE_MAP_BASE: usize = 0x1_0000;
pub const TILE_MAP_SIZE: usize = 0x4000;
pub const TILE_MAP_ROW_STRIDE: usize = 0x80;
pub const SURFACE_COIN: u8 = 0x1A;
pub const SURFACE_ITEM_PANEL_UNUSED: u8 = 0x14;

pub const VRAM_UPDATE_FLAG_ADDR: usize = 0x1E20;
pub const VRAM_UPDATE_GROUP_ADDR: usize = 0x1E22;
pub const ITEM_BOX_VRAM_DELTA: [i16; 4] = [0x0000, -1, -128, -129];

pub const HAZARD_SLOT_BASES: [usize; 4] = [0x1840, 0x18C0, 0x1940, 0x19C0];
pub const HAZARD_SLOT_SIZE: usize = 0x80;

pub const FISH_POS0_LO_OFFSET: usize = 0x1F;
pub const FISH_POS0_HI_OFFSET: usize = 0x20;
pub const FISH_VEL0_OFFSET: usize = 0x26;
pub const FISH_POS1_FRAC_OFFSET: usize = 0x17;
pub const FISH_POS1_WORD_OFFSET: usize = 0x18;
pub const FISH_VEL1_OFFSET: usize = 0x22;
pub const FISH_TGT1_OFFSET: usize = 0x28;
pub const FISH_POS2_FRAC_OFFSET: usize = 0x1B;
pub const FISH_POS2_WORD_OFFSET: usize = 0x1C;
pub const FISH_VEL2_OFFSET: usize = 0x24;
pub const FISH_TGT2_OFFSET: usize = 0x2A;

pub const THWOMP_POS_FRAC_OFFSET: usize = 0x1F;
pub const THWOMP_POS_WORD_OFFSET: usize = 0x20;
pub const THWOMP_VEL_OFFSET: usize = 0x26;
pub const THWOMP_STATE_OFFSET: usize = 0x28;
pub const THWOMP_CD_OFFSET: usize = 0x2A;

pub const MOLE_POS_LO_OFFSET: usize = 0x1F;
pub const MOLE_POS_HI_OFFSET: usize = 0x20;
pub const MOLE_VEL_OFFSET: usize = 0x26;
pub const MOLE_STATE_OFFSET: usize = 0x28;
pub const MOLE_CD_OFFSET: usize = 0x2A;

pub const TRACK_ID_FISH_KB1: u8 = 10;
pub const TRACK_ID_FISH_KB2: u8 = 16;
pub const TRACK_IDS_THWOMP: [u8; 4] = [3, 8, 13, 19];
pub const TRACK_IDS_MOLES: [u8; 2] = [7, 15];

pub fn racer_offset(kart_slot: u8) -> usize {
    RACER_STRUCT_BASE + (kart_slot as usize) * RACER_STRUCT_SIZE
}

pub fn kart_slot_from_owner_addr(addr: u16) -> Option<u8> {
    let addr = addr as usize;
    if addr < RACER_STRUCT_BASE {
        return None;
    }
    let offset = addr - RACER_STRUCT_BASE;
    if offset % RACER_STRUCT_SIZE != 0 {
        return None;
    }
    let kart_slot = offset / RACER_STRUCT_SIZE;
    if kart_slot < NUM_KART_SLOTS as usize {
        Some(kart_slot as u8)
    } else {
        None
    }
}

pub fn is_valid_item_type(raw: u8) -> bool {
    raw < NUM_ITEM_TYPES
}

pub type ServerSlot = u8;
pub type KartSlot = u8;

#[derive(Clone)]
pub struct SlotMap {
    map: std::collections::HashMap<ServerSlot, KartSlot>,
    used_kart_slots: [bool; NUM_KART_SLOTS as usize],
}

impl SlotMap {
    pub fn new() -> Self {
        let mut used_kart_slots = [false; NUM_KART_SLOTS as usize];
        used_kart_slots[LOCAL_KART_SLOT as usize] = true;
        Self { map: std::collections::HashMap::new(), used_kart_slots }
    }

    pub fn get(&self, server_slot: ServerSlot) -> Option<KartSlot> {
        self.map.get(&server_slot).copied()
    }

    pub fn assign(&mut self, server_slot: ServerSlot) -> Option<KartSlot> {
        if let Some(&kart_slot) = self.map.get(&server_slot) {
            return Some(kart_slot);
        }
        for kart_slot in 0..NUM_KART_SLOTS {
            if !self.used_kart_slots[kart_slot as usize] {
                self.used_kart_slots[kart_slot as usize] = true;
                self.map.insert(server_slot, kart_slot);
                return Some(kart_slot);
            }
        }
        eprintln!(
            "wram::SlotMap::assign: no free kart slot available for server_slot {server_slot} (map has {} entries)",
            self.map.len()
        );
        None
    }

    pub fn server_slot_for_kart(&self, kart_slot: KartSlot) -> Option<ServerSlot> {
        self.map.iter().find(|&(_, &k)| k == kart_slot).map(|(&s, _)| s)
    }

    pub fn free(&mut self, server_slot: ServerSlot) {
        if let Some(kart_slot) = self.map.remove(&server_slot) {
            self.used_kart_slots[kart_slot as usize] = false;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ServerSlot, KartSlot)> + '_ {
        self.map.iter().map(|(&server_slot, &kart_slot)| (server_slot, kart_slot))
    }
}

impl Default for SlotMap {
    fn default() -> Self {
        Self::new()
    }
}