use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};

use crate::wram;

pub fn write_mailbox_bit(ram: &mut [u8], bit: u8, value: bool) {
    if wram::MAILBOX_ADDR >= ram.len() {
        eprintln!(
            "writer: mailbox write out of bounds: addr {:#x} >= ram len {:#x}",
            wram::MAILBOX_ADDR,
            ram.len()
        );
        return;
    }
    if value {
        ram[wram::MAILBOX_ADDR] |= bit;
    } else {
        ram[wram::MAILBOX_ADDR] &= !bit;
    }
}

pub fn write_rng_seed(ram: &mut [u8], seed: u16) {
    if wram::RNG_ADDR + 1 >= ram.len() {
        eprintln!(
            "writer: RNG write out of bounds: addr {:#x} >= ram len {:#x}",
            wram::RNG_ADDR,
            ram.len()
        );
        return;
    }
    let bytes = seed.to_le_bytes();
    ram[wram::RNG_ADDR] = bytes[0];
    ram[wram::RNG_ADDR + 1] = bytes[1];
}

#[allow(clippy::too_many_arguments)]
pub fn write_fish_state(
    ram: &mut [u8],
    slot_index: usize,
    pos0: i16,
    vel0: i16,
    pos1: i16,
    frac1: u8,
    vel1: i16,
    tgt1: i16,
    pos2: i16,
    frac2: u8,
    vel2: i16,
    tgt2: i16,
) {
    let Some(&base) = wram::HAZARD_SLOT_BASES.get(slot_index) else {
        eprintln!("writer: write_fish_state: bad slot_index {slot_index} (expected 0..4)");
        return;
    };
    let end = base + wram::HAZARD_SLOT_SIZE;
    if end > ram.len() {
        eprintln!(
            "writer: fish write out of bounds: slot base {base:#x} + size {:#x} > ram len {:#x}",
            wram::HAZARD_SLOT_SIZE,
            ram.len()
        );
        return;
    }

    let pos0_bytes = pos0.to_le_bytes();
    ram[base + wram::FISH_POS0_LO_OFFSET] = pos0_bytes[0];
    ram[base + wram::FISH_POS0_HI_OFFSET] = pos0_bytes[1];

    let vel0_bytes = vel0.to_le_bytes();
    ram[base + wram::FISH_VEL0_OFFSET] = vel0_bytes[0];
    ram[base + wram::FISH_VEL0_OFFSET + 1] = vel0_bytes[1];

    ram[base + wram::FISH_POS1_FRAC_OFFSET] = frac1;
    let pos1_bytes = pos1.to_le_bytes();
    ram[base + wram::FISH_POS1_WORD_OFFSET] = pos1_bytes[0];
    ram[base + wram::FISH_POS1_WORD_OFFSET + 1] = pos1_bytes[1];

    let vel1_bytes = vel1.to_le_bytes();
    ram[base + wram::FISH_VEL1_OFFSET] = vel1_bytes[0];
    ram[base + wram::FISH_VEL1_OFFSET + 1] = vel1_bytes[1];

    let tgt1_bytes = tgt1.to_le_bytes();
    ram[base + wram::FISH_TGT1_OFFSET] = tgt1_bytes[0];
    ram[base + wram::FISH_TGT1_OFFSET + 1] = tgt1_bytes[1];

    ram[base + wram::FISH_POS2_FRAC_OFFSET] = frac2;
    let pos2_bytes = pos2.to_le_bytes();
    ram[base + wram::FISH_POS2_WORD_OFFSET] = pos2_bytes[0];
    ram[base + wram::FISH_POS2_WORD_OFFSET + 1] = pos2_bytes[1];

    let vel2_bytes = vel2.to_le_bytes();
    ram[base + wram::FISH_VEL2_OFFSET] = vel2_bytes[0];
    ram[base + wram::FISH_VEL2_OFFSET + 1] = vel2_bytes[1];

    let tgt2_bytes = tgt2.to_le_bytes();
    ram[base + wram::FISH_TGT2_OFFSET] = tgt2_bytes[0];
    ram[base + wram::FISH_TGT2_OFFSET + 1] = tgt2_bytes[1];

    const SHADOW_OFFSET: usize = 0x40;
    ram[base + wram::FISH_POS0_LO_OFFSET + SHADOW_OFFSET] = pos0_bytes[0];
    ram[base + wram::FISH_POS0_HI_OFFSET + SHADOW_OFFSET] = pos0_bytes[1];
    ram[base + wram::FISH_VEL0_OFFSET + SHADOW_OFFSET] = vel0_bytes[0];
    ram[base + wram::FISH_VEL0_OFFSET + SHADOW_OFFSET + 1] = vel0_bytes[1];
    ram[base + wram::FISH_VEL1_OFFSET + SHADOW_OFFSET] = vel1_bytes[0];
    ram[base + wram::FISH_VEL1_OFFSET + SHADOW_OFFSET + 1] = vel1_bytes[1];
    ram[base + wram::FISH_VEL2_OFFSET + SHADOW_OFFSET] = vel2_bytes[0];
    ram[base + wram::FISH_VEL2_OFFSET + SHADOW_OFFSET + 1] = vel2_bytes[1];
}

pub fn write_thwomp_state(
    ram: &mut [u8],
    slot_index: usize,
    pos: i16,
    frac: u8,
    vel: i16,
    state: u8,
    remaining: i32,
) {
    let Some(&base) = wram::HAZARD_SLOT_BASES.get(slot_index) else {
        eprintln!("writer: write_thwomp_state: bad slot_index {slot_index} (expected 0..4)");
        return;
    };
    let end = base + wram::HAZARD_SLOT_SIZE;
    if end > ram.len() {
        eprintln!(
            "writer: thwomp write out of bounds: slot base {base:#x} + size {:#x} > ram len {:#x}",
            wram::HAZARD_SLOT_SIZE,
            ram.len()
        );
        return;
    }

    ram[base + wram::THWOMP_POS_FRAC_OFFSET] = frac;
    let pos_bytes = pos.to_le_bytes();
    ram[base + wram::THWOMP_POS_WORD_OFFSET] = pos_bytes[0];
    ram[base + wram::THWOMP_POS_WORD_OFFSET + 1] = pos_bytes[1];

    let vel_bytes = vel.to_le_bytes();
    ram[base + wram::THWOMP_VEL_OFFSET] = vel_bytes[0];
    ram[base + wram::THWOMP_VEL_OFFSET + 1] = vel_bytes[1];

    ram[base + wram::THWOMP_STATE_OFFSET] = state;

    let remaining_bytes = (remaining as i16).to_le_bytes();
    ram[base + wram::THWOMP_CD_OFFSET] = remaining_bytes[0];
    ram[base + wram::THWOMP_CD_OFFSET + 1] = remaining_bytes[1];
}

pub fn write_mole_state(ram: &mut [u8], slot_index: usize, pos: i16, vel: i16, state: u8, cd: i32) {
    let Some(&base) = wram::HAZARD_SLOT_BASES.get(slot_index) else {
        eprintln!("writer: write_mole_state: bad slot_index {slot_index} (expected 0..4)");
        return;
    };
    let end = base + wram::HAZARD_SLOT_SIZE;
    if end > ram.len() {
        eprintln!(
            "writer: mole write out of bounds: slot base {base:#x} + size {:#x} > ram len {:#x}",
            wram::HAZARD_SLOT_SIZE,
            ram.len()
        );
        return;
    }

    let pos_bytes = pos.to_le_bytes();
    ram[base + wram::MOLE_POS_LO_OFFSET] = pos_bytes[0];
    ram[base + wram::MOLE_POS_HI_OFFSET] = pos_bytes[1];

    let vel_bytes = vel.to_le_bytes();
    ram[base + wram::MOLE_VEL_OFFSET] = vel_bytes[0];
    ram[base + wram::MOLE_VEL_OFFSET + 1] = vel_bytes[1];

    ram[base + wram::MOLE_STATE_OFFSET] = state;

    let cd_bytes = (cd as i16).to_le_bytes();
    ram[base + wram::MOLE_CD_OFFSET] = cd_bytes[0];
    ram[base + wram::MOLE_CD_OFFSET + 1] = cd_bytes[1];
}

pub fn inject_track_index(ram: &mut [u8], track_index: u8) {
    if wram::RACE_NUMBER_ADDR + 1 >= ram.len() || wram::CUP_NUMBER_ADDR + 1 >= ram.len() {
        eprintln!(
            "writer: track-index write out of bounds: addr {:#x}/{:#x} >= ram len {:#x}",
            wram::RACE_NUMBER_ADDR,
            wram::CUP_NUMBER_ADDR,
            ram.len()
        );
        return;
    }
    ram[wram::RACE_NUMBER_ADDR] = track_index;
    ram[wram::RACE_NUMBER_ADDR + 1] = 0;
    ram[wram::CUP_NUMBER_ADDR] = 0;
    ram[wram::CUP_NUMBER_ADDR + 1] = 0;
}

const EXCLUDED_RANGES: &[(u8, u8)] = &[
    (0x00, 0x13),
    (0x14, 0x15),
    (0x2C, 0x3F),
    (0x40, 0x41),
    (0x74, 0x7F),
    (0x98, 0x9F),
    (0xA4, 0xA5),
    (0xB6, 0xBD),
    (0xD2, 0xD3),
    (0xDE, 0xDF),
    (0xE6, 0xE6),
];

fn is_excluded(offset: u8) -> bool {
    EXCLUDED_RANGES.iter().any(|&(lo, hi)| (lo..=hi).contains(&offset))
}

pub fn apply_incoming_racer_data(ram: &mut [u8], kart_slot: u8, payload: &[u8]) {
    if payload.len() != wram::RACER_STRUCT_SIZE {
        eprintln!(
            "writer: apply_incoming_racer_data: payload is {} bytes, expected {} -- dropping",
            payload.len(),
            wram::RACER_STRUCT_SIZE
        );
        return;
    }
    let base = wram::racer_offset(kart_slot);
    if base + wram::RACER_STRUCT_SIZE > ram.len() {
        eprintln!(
            "writer: apply_incoming_racer_data: kart_slot {kart_slot} struct falls outside WRAM (base {base:#x})"
        );
        return;
    }
    for i in 0..wram::RACER_STRUCT_SIZE {
        if !is_excluded(i as u8) {
            ram[base + i] = payload[i];
        }
    }
}

pub fn extract_local_racer_data(ram: &[u8]) -> [u8; wram::RACER_STRUCT_SIZE] {
    let base = wram::racer_offset(wram::LOCAL_KART_SLOT);
    let mut out = [0u8; wram::RACER_STRUCT_SIZE];
    if base + wram::RACER_STRUCT_SIZE > ram.len() {
        eprintln!(
            "writer: extract_local_racer_data: local racer struct falls outside WRAM (base {base:#x}) -- returning zeroed struct"
        );
        return out;
    }
    out.copy_from_slice(&ram[base..base + wram::RACER_STRUCT_SIZE]);
    out
}

const POSITION_FIELD_RANGES: &[(u8, u8)] = &[(0x16, 0x21), (0x22, 0x27), (0x2A, 0x2B), (0x40, 0x41)];

fn is_position_field(offset: u8) -> bool {
    POSITION_FIELD_RANGES.iter().any(|&(lo, hi)| (lo..=hi).contains(&offset))
}

fn find_kart_slot_with_rank(ram: &[u8], target_rank_raw: u16) -> Option<u8> {
    for kart_slot in 0..wram::NUM_KART_SLOTS {
        let base = wram::racer_offset(kart_slot);
        if base + 0x42 > ram.len() {
            continue;
        }
        let rank_raw = u16::from_le_bytes([ram[base + 0x40], ram[base + 0x41]]);
        if rank_raw == target_rank_raw {
            return Some(kart_slot);
        }
    }
    None
}

fn apply_position_to_slot(ram: &mut [u8], target_kart_slot: u8, desired_start_pos: u8) {
    if desired_start_pos == 0 || desired_start_pos > wram::NUM_KART_SLOTS {
        eprintln!(
            "writer: apply_position_to_slot: desired_start_pos {desired_start_pos} out of range 1-{}, leaving kart slot {target_kart_slot} untouched",
            wram::NUM_KART_SLOTS
        );
        return;
    }

    let target_rank_raw = ((desired_start_pos - 1) as u16) * 2;

    let Some(source_slot) = find_kart_slot_with_rank(ram, target_rank_raw) else {
        eprintln!(
            "writer: apply_position_to_slot: no kart slot currently holds the rank for start_pos {desired_start_pos} -- race-1 default grid setup may not have run yet, leaving kart slot {target_kart_slot} untouched"
        );
        return;
    };

    if source_slot == target_kart_slot {
        return;
    }

    let target_base = wram::racer_offset(target_kart_slot);
    let source_base = wram::racer_offset(source_slot);
    if target_base.max(source_base) + wram::RACER_STRUCT_SIZE > ram.len() {
        eprintln!("writer: apply_position_to_slot: struct falls outside WRAM, aborting swap");
        return;
    }

    for i in 0..wram::RACER_STRUCT_SIZE as u16 {
        let i = i as u8;
        if is_position_field(i) {
            ram.swap(target_base + i as usize, source_base + i as usize);
        }
    }
}

const REMOVED_FLAG_SET_1: u16 = 0x0000;
const REMOVED_SCREEN_NUMBER: u16 = 0xFFFF;

fn remove_unoccupied_cpu(ram: &mut [u8], kart_slot: u8) -> Option<(u16, u16)> {
    let base = wram::racer_offset(kart_slot);
    if base + 0xD4 > ram.len() {
        eprintln!("writer: remove_unoccupied_cpu: kart slot {kart_slot} struct falls outside WRAM, skipping");
        return None;
    }
    let original_flag = u16::from_le_bytes([ram[base + 0x10], ram[base + 0x11]]);
    let original_screen = u16::from_le_bytes([ram[base + 0xD2], ram[base + 0xD3]]);

    let flag_bytes = REMOVED_FLAG_SET_1.to_le_bytes();
    ram[base + 0x10] = flag_bytes[0];
    ram[base + 0x11] = flag_bytes[1];
    let screen_bytes = REMOVED_SCREEN_NUMBER.to_le_bytes();
    ram[base + 0xD2] = screen_bytes[0];
    ram[base + 0xD3] = screen_bytes[1];

    Some((original_flag, original_screen))
}

pub fn remove_unoccupied_cpus(ram: &mut [u8], occupied_kart_slots: &[u8]) -> HashMap<u8, (u16, u16)> {
    let mut occupied = [false; wram::NUM_KART_SLOTS as usize];
    for &kart_slot in occupied_kart_slots {
        if (kart_slot as usize) < occupied.len() {
            occupied[kart_slot as usize] = true;
        }
    }
    let mut disabled = HashMap::new();
    for kart_slot in 0..wram::NUM_KART_SLOTS {
        if !occupied[kart_slot as usize] {
            if let Some(original) = remove_unoccupied_cpu(ram, kart_slot) {
                disabled.insert(kart_slot, original);
            }
        }
    }
    disabled
}

pub fn restore_cpu_slot(ram: &mut [u8], kart_slot: u8, original: (u16, u16)) {
    let base = wram::racer_offset(kart_slot);
    if base + 0xD4 > ram.len() {
        eprintln!("writer: restore_cpu_slot: kart slot {kart_slot} struct falls outside WRAM, skipping");
        return;
    }
    let (original_flag, original_screen) = original;
    let flag_bytes = original_flag.to_le_bytes();
    ram[base + 0x10] = flag_bytes[0];
    ram[base + 0x11] = flag_bytes[1];
    let screen_bytes = original_screen.to_le_bytes();
    ram[base + 0xD2] = screen_bytes[0];
    ram[base + 0xD3] = screen_bytes[1];
}

pub fn run_startup_placement(ram: &mut [u8], my_start_pos: u8, others: &[(u8, u8)]) {
    apply_position_to_slot(ram, wram::LOCAL_KART_SLOT, my_start_pos);

    for &(their_kart_slot, their_start_pos) in others {
        if their_kart_slot == wram::LOCAL_KART_SLOT {
            eprintln!(
                "writer: run_startup_placement: 'others' included kart slot {} (LOCAL_KART_SLOT) -- skipping, this should never happen",
                wram::LOCAL_KART_SLOT
            );
            continue;
        }
        apply_position_to_slot(ram, their_kart_slot, their_start_pos);
    }

    let occupied_kart_slots: Vec<u8> =
        std::iter::once(wram::LOCAL_KART_SLOT).chain(others.iter().map(|&(slot, _)| slot)).collect();
    remove_unoccupied_cpus(ram, &occupied_kart_slots);
}

fn apply_non_collision(ram: &mut [u8], kart_slot: u8) {
    let base = wram::racer_offset(kart_slot);
    let offset = base + wram::Z_POSITION_OFFSET;
    if offset + 1 >= ram.len() {
        eprintln!("writer: apply_non_collision: kart slot {kart_slot} Z-position offset falls outside WRAM, skipping");
        return;
    }
    let bytes = wram::Z_POSITION_MAX.to_le_bytes();
    ram[offset] = bytes[0];
    ram[offset + 1] = bytes[1];
}

pub fn apply_non_collision_to_cpus(ram: &mut [u8], human_kart_slots: &[u8]) {
    for kart_slot in 0..wram::NUM_KART_SLOTS {
        if human_kart_slots.contains(&kart_slot) {
            continue;
        }
        apply_non_collision(ram, kart_slot);
    }
}

fn has_finished(ram: &[u8], kart_slot: u8) -> bool {
    let lap_byte_offset = wram::racer_offset(kart_slot) + wram::CURRENT_LAP_OFFSET;
    lap_byte_offset < ram.len() && ram[lap_byte_offset] == wram::FINISHED_LAP_VALUE
}

fn all_humans_finished(ram: &[u8], human_kart_slots: &[u8]) -> bool {
    !human_kart_slots.is_empty() && human_kart_slots.iter().all(|&slot| has_finished(ram, slot))
}

const CHECKPOINT_OFFSET: usize = 0xC0;
const FINISH_QUEUE_BASE: usize = 0x0E90;
const FINISH_QUEUE_COUNT_ADDR: usize = 0x0E8E;
const FINISH_QUEUE_CAPACITY: u16 = 8;

fn force_finish_cpu(ram: &mut [u8], kart_slot: u8) -> bool {
    let base = wram::racer_offset(kart_slot);
    let checkpoint_offset = base + CHECKPOINT_OFFSET;
    let lap_offset = base + wram::CURRENT_LAP_OFFSET;
    if lap_offset >= ram.len() || checkpoint_offset >= ram.len() {
        eprintln!("writer: force_finish_cpu: kart slot {kart_slot} struct falls outside WRAM, skipping");
        return false;
    }

    if FINISH_QUEUE_COUNT_ADDR + 1 >= ram.len() {
        eprintln!("writer: force_finish_cpu: finish-queue count address falls outside WRAM, skipping enqueue");
        return false;
    }
    let count = u16::from_le_bytes([ram[FINISH_QUEUE_COUNT_ADDR], ram[FINISH_QUEUE_COUNT_ADDR + 1]]);
    if count >= FINISH_QUEUE_CAPACITY * 2 {
        eprintln!(
            "writer: force_finish_cpu: finish queue already full (count {count}), leaving kart slot {kart_slot} completely untouched -- will retry once the queue drains"
        );
        return false;
    }
    let slot_addr = FINISH_QUEUE_BASE + count as usize;
    if slot_addr + 1 >= ram.len() {
        eprintln!("writer: force_finish_cpu: finish-queue slot falls outside WRAM, skipping enqueue");
        return false;
    }
    let base_bytes = (base as u16).to_le_bytes();
    ram[slot_addr] = base_bytes[0];
    ram[slot_addr + 1] = base_bytes[1];
    let new_count = (count + 2).to_le_bytes();
    ram[FINISH_QUEUE_COUNT_ADDR] = new_count[0];
    ram[FINISH_QUEUE_COUNT_ADDR + 1] = new_count[1];

    ram[checkpoint_offset] = 0x00;
    ram[lap_offset] = wram::FINISHED_LAP_VALUE;

    true
}

#[derive(Default)]
pub struct AutoFinisher {
    done: bool,
}

impl AutoFinisher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.done = false;
    }

    pub fn poll(&mut self, ram: &mut [u8], human_kart_slots: &[u8]) {
        if self.done {
            return;
        }
        if !all_humans_finished(ram, human_kart_slots) {
            return;
        }

        let mut all_succeeded = true;
        for kart_slot in 0..wram::NUM_KART_SLOTS {
            if human_kart_slots.contains(&kart_slot) {
                continue;
            }
            if has_finished(ram, kart_slot) {
                continue;
            }
            if !force_finish_cpu(ram, kart_slot) {
                all_succeeded = false;
            }
        }
        self.done = all_succeeded;
    }

    pub fn force_finish_all(&mut self, ram: &mut [u8]) {
        if self.done {
            return;
        }
        let mut all_succeeded = true;
        for kart_slot in 0..wram::NUM_KART_SLOTS {
            if has_finished(ram, kart_slot) {
                continue;
            }
            if !force_finish_cpu(ram, kart_slot) {
                all_succeeded = false;
            }
        }
        self.done = all_succeeded;
    }
}

const PACKET_TYPE_RACER: u8 = b'R';
const PACKET_HEADER_LEN: usize = 1 + 1 + 2 + 2;

#[derive(Clone, PartialEq, Eq)]
pub struct RacerPacket {
    pub server_slot: wram::ServerSlot,
    pub sequence: u16,
    pub payload: Vec<u8>,
}

pub fn encode_packet(server_slot: wram::ServerSlot, sequence: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(PACKET_HEADER_LEN + payload.len());
    out.push(PACKET_TYPE_RACER);
    out.push(server_slot);
    out.extend_from_slice(&sequence.to_be_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn encode_outgoing_packet(token: &[u8; 8], server_slot: wram::ServerSlot, sequence: u16, payload: &[u8]) -> Vec<u8> {
    let inner = encode_packet(server_slot, sequence, payload);
    let mut out = Vec::with_capacity(1 + token.len() + inner.len() - 1);
    out.push(inner[0]);
    out.extend_from_slice(token);
    out.extend_from_slice(&inner[1..]);
    out
}

pub fn decode_packet(bytes: &[u8]) -> Option<RacerPacket> {
    if bytes.len() < PACKET_HEADER_LEN {
        eprintln!(
            "writer: decode_packet: packet too short ({} bytes, need at least {})",
            bytes.len(),
            PACKET_HEADER_LEN
        );
        return None;
    }
    if bytes[0] != PACKET_TYPE_RACER {
        eprintln!("writer: decode_packet: unrecognized packet type {:#x}, dropping", bytes[0]);
        return None;
    }
    let server_slot = bytes[1];
    let sequence = u16::from_be_bytes([bytes[2], bytes[3]]);
    let len = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let payload = &bytes[PACKET_HEADER_LEN..];
    if payload.len() != len {
        eprintln!(
            "writer: decode_packet: declared length {len} doesn't match actual payload {} bytes, dropping",
            payload.len()
        );
        return None;
    }
    Some(RacerPacket { server_slot, sequence, payload: payload.to_vec() })
}

#[derive(Default)]
pub struct SequenceGate {
    last_seen: HashMap<wram::ServerSlot, u16>,
}

impl SequenceGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accept(&mut self, server_slot: wram::ServerSlot, sequence: u16) -> bool {
        match self.last_seen.get(&server_slot) {
            Some(&last) if (sequence.wrapping_sub(last) as i16) <= 0 => false,
            _ => {
                self.last_seen.insert(server_slot, sequence);
                true
            }
        }
    }
}

pub struct RacerMirrorSocket {
    socket: UdpSocket,
    server_addr: SocketAddr,
    token: [u8; 8],
    sequence_gate: SequenceGate,
    next_sequence: u16,
    received_this_race: HashSet<wram::ServerSlot>,
}

impl RacerMirrorSocket {
    pub fn new(bind_addr: &str, server_addr: &str, token: [u8; 8]) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        let server_addr: SocketAddr = server_addr
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad server addr: {e}")))?;
        Ok(Self {
            socket,
            server_addr,
            token,
            sequence_gate: SequenceGate::new(),
            next_sequence: 0,
            received_this_race: HashSet::new(),
        })
    }

    pub fn reset_for_new_race(&mut self) {
        self.received_this_race.clear();
        self.sequence_gate = SequenceGate::new();
    }

    pub fn hide_pending_remote_slots(&mut self, ram: &mut [u8], known_remotes: &[(wram::ServerSlot, u8)]) {
        for &(server_slot, kart_slot) in known_remotes {
            if self.received_this_race.contains(&server_slot) {
                continue;
            }
            apply_non_collision(ram, kart_slot);
        }
    }

    pub fn send_local(&mut self, ram: &[u8], our_server_slot: wram::ServerSlot) {
        let payload = extract_local_racer_data(ram);
        let packet = encode_outgoing_packet(&self.token, our_server_slot, self.next_sequence, &payload);
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if let Err(e) = self.socket.send_to(&packet, self.server_addr) {
            eprintln!("RacerMirrorSocket::send_local: send failed: {e}");
        }
    }

    pub fn poll_and_apply(&mut self, ram: &mut [u8], slot_map: &wram::SlotMap, our_server_slot: wram::ServerSlot) {
        let mut buf = [0u8; 2048];
        loop {
            let (len, _) = match self.socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    eprintln!("RacerMirrorSocket::poll_and_apply: recv error: {e}");
                    break;
                }
            };
            let Some(packet) = decode_packet(&buf[..len]) else {
                continue;
            };
            if packet.server_slot == our_server_slot {
                continue;
            }
            if !self.sequence_gate.accept(packet.server_slot, packet.sequence) {
                continue;
            }
            if let Some(kart_slot) = slot_map.get(packet.server_slot) {
                self.received_this_race.insert(packet.server_slot);
                apply_incoming_racer_data(ram, kart_slot, &packet.payload);
            }
        }
    }
}

struct DecodedThrow {
    item_type: u8,
    alt_direction: bool,
}

fn item_type_from_field_value(raw: u8) -> Option<DecodedThrow> {
    if raw % 2 != 0 {
        return None;
    }
    let mut y = raw / 2;

    let alt_direction = y >= 6;
    if alt_direction {
        if y > 7 {
            return None;
        }
        y -= 6;
    }

    if wram::is_valid_item_type(y) {
        Some(DecodedThrow { item_type: y, alt_direction })
    } else {
        None
    }
}

fn rd16(ram: &[u8], addr: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*ram.get(addr)?, *ram.get(addr + 1)?]))
}

fn wr16(ram: &mut [u8], addr: usize, v: u16) -> bool {
    if addr + 1 >= ram.len() {
        return false;
    }
    let b = v.to_le_bytes();
    ram[addr] = b[0];
    ram[addr + 1] = b[1];
    true
}

#[derive(Clone, Copy)]
struct ItemSlotView {
    base: usize,
    occupied: bool,
    owner_raw: u16,
    type_raw: u8,
    collision_timer: u16,
}

impl ItemSlotView {
    fn is_hit(&self) -> bool {
        self.occupied && self.collision_timer & wram::ITEM_HIT_FLAG != 0
    }

    fn owner_kart_slot(&self) -> Option<u8> {
        if self.occupied {
            wram::kart_slot_from_owner_addr(self.owner_raw)
        } else {
            None
        }
    }

    fn is_owned_by(&self, kart_slot: u8) -> bool {
        let target_base = wram::racer_offset(kart_slot) as u16;
        let target_end = target_base + wram::RACER_STRUCT_SIZE as u16;
        self.occupied && self.owner_raw >= target_base && self.owner_raw < target_end
    }
}

fn read_item_slot(ram: &[u8], slot_index: usize, context: &str) -> Option<ItemSlotView> {
    let base = wram::ITEM_SLOTS_ADDR + slot_index * wram::ITEM_SLOT_SIZE;
    if base + wram::ITEM_SLOT_SIZE > ram.len() {
        eprintln!("writer: {context}: item slot {slot_index} falls outside WRAM, skipping");
        return None;
    }
    Some(ItemSlotView {
        base,
        occupied: rd16(ram, base + wram::ITEM_SLOT_OCCUPIED_OFFSET).unwrap_or(0) & wram::ITEM_SLOT_OCCUPIED_BIT != 0,
        owner_raw: rd16(ram, base + wram::ITEM_SLOT_OWNER_OFFSET).unwrap_or(0),
        type_raw: ram[base + wram::ITEM_SLOT_TYPE_OFFSET],
        collision_timer: rd16(ram, base + wram::ITEM_SLOT_COLLISION_TIMER_OFFSET).unwrap_or(0),
    })
}

fn find_item_slot_owned_by(ram: &[u8], kart_slot: u8, context: &str) -> Option<ItemSlotView> {
    if kart_slot >= wram::NUM_KART_SLOTS {
        eprintln!("writer: {context}: kart_slot {kart_slot} out of range -- ignoring");
        return None;
    }
    (0..wram::NUM_ITEM_SLOTS)
        .filter_map(|slot_index| read_item_slot(ram, slot_index, context))
        .find(|slot| slot.is_owned_by(kart_slot))
}

fn remove_item_like_engine(ram: &mut [u8], base: usize) {
    const DP_LIMIT: usize = 0x2000;
    fn rd(ram: &[u8], a: usize) -> u16 {
        if a + 1 < DP_LIMIT && a + 1 < ram.len() {
            u16::from_le_bytes([ram[a], ram[a + 1]])
        } else {
            0
        }
    }
    fn wr(ram: &mut [u8], a: usize, v: u16) {
        if a + 1 < DP_LIMIT && a + 1 < ram.len() {
            ram[a..a + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    let unlink = |ram: &mut [u8], n: usize, p: usize| {
        let next = rd(ram, base + n) as usize;
        let prev = rd(ram, base + p) as usize;
        wr(ram, prev + n, next as u16);
        wr(ram, next + p, prev as u16);
    };

    unlink(ram, 0x00, 0x02);
    let occupied = rd(ram, base + wram::ITEM_SLOT_OCCUPIED_OFFSET) & wram::ITEM_SLOT_OCCUPIED_BIT != 0;
    unlink(ram, 0x08, 0x0A);
    if occupied {
        wr(ram, base + wram::ITEM_SLOT_OCCUPIED_OFFSET, 0);
        unlink(ram, 0x60, 0x62);
    }
}

pub fn item_owned_by_is_in_hit_animation(ram: &[u8], kart_slot: u8) -> bool {
    if kart_slot >= wram::NUM_KART_SLOTS {
        return false;
    }
    (0..wram::NUM_ITEM_SLOTS)
        .filter_map(|slot_index| read_item_slot(ram, slot_index, "item_owned_by_is_in_hit_animation"))
        .any(|slot| slot.is_owned_by(kart_slot) && slot.is_hit())
}

pub fn force_free_item_slot_owned_by(ram: &mut [u8], kart_slot: u8) -> Option<usize> {
    let slot = find_item_slot_owned_by(ram, kart_slot, "force_free_item_slot_owned_by")?;
    remove_item_like_engine(ram, slot.base);
    Some((slot.base - wram::ITEM_SLOTS_ADDR) / wram::ITEM_SLOT_SIZE)
}

pub fn force_item_into_hit_animation_owned_by(ram: &mut [u8], kart_slot: u8) -> Option<usize> {
    let slot = find_item_slot_owned_by(ram, kart_slot, "force_item_into_hit_animation_owned_by")?;
    if slot.is_hit() {
        return None;
    }
    wr16(ram, slot.base + wram::ITEM_SLOT_Z_VELOCITY_OFFSET, wram::ITEM_HIT_Z_VELOCITY_KICK);
    wr16(ram, slot.base + wram::ITEM_SLOT_COLLISION_TIMER_OFFSET, wram::ITEM_HIT_COLLISION_TIMER_VALUE);
    Some((slot.base - wram::ITEM_SLOTS_ADDR) / wram::ITEM_SLOT_SIZE)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LocalThrow {
    pub item_type: u8,
    pub alt_direction: bool,
    pub evicted_remote_kart_slot: Option<u8>,
}

#[derive(Default)]
pub struct ThrowWatcher {
    prev_owner_kart_slot: [Option<u8>; wram::NUM_ITEM_SLOTS],
    prev_type_raw: [u8; wram::NUM_ITEM_SLOTS],
}

impl ThrowWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn poll(&mut self, ram: &[u8]) -> Option<LocalThrow> {
        let mut result = None;
        for slot_index in 0..wram::NUM_ITEM_SLOTS {
            let Some(slot) = read_item_slot(ram, slot_index, "ThrowWatcher::poll") else { continue };

            let cur_owner = slot.owner_kart_slot();
            let cur_type_raw = slot.type_raw;

            let prev_owner = self.prev_owner_kart_slot[slot_index];
            let prev_type_raw = self.prev_type_raw[slot_index];

            let is_new_throw_by_us = cur_owner == Some(wram::LOCAL_KART_SLOT)
                && (prev_owner != Some(wram::LOCAL_KART_SLOT) || prev_type_raw != cur_type_raw);

            self.prev_owner_kart_slot[slot_index] = cur_owner;
            self.prev_type_raw[slot_index] = cur_type_raw;

            if !is_new_throw_by_us {
                continue;
            }

            match item_type_from_field_value(cur_type_raw) {
                Some(DecodedThrow { item_type, alt_direction }) => {
                    let evicted_remote_kart_slot = match prev_owner {
                        Some(k) if k != wram::LOCAL_KART_SLOT => Some(k),
                        _ => None,
                    };
                    result = Some(LocalThrow { item_type, alt_direction, evicted_remote_kart_slot });
                }
                None => eprintln!(
                    "writer: ThrowWatcher::poll: slot {slot_index} newly thrown by us, but +0x70={cur_type_raw:#04x} isn't a recognized item type -- not reporting"
                ),
            }
        }
        result
    }
}

#[derive(Default)]
pub struct DespawnWatcher {
    prev_hit: [bool; wram::NUM_ITEM_SLOTS],
    suppressed: [bool; wram::NUM_ITEM_SLOTS],
}

impl DespawnWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn suppress_slot(&mut self, slot_index: usize) {
        if slot_index < wram::NUM_ITEM_SLOTS {
            self.suppressed[slot_index] = true;
        }
    }

    pub fn poll(&mut self, ram: &[u8]) -> Vec<u8> {
        let mut despawning = Vec::new();
        for slot_index in 0..wram::NUM_ITEM_SLOTS {
            let Some(slot) = read_item_slot(ram, slot_index, "DespawnWatcher::poll") else { continue };

            let cur_owner = slot.owner_kart_slot();
            let cur_hit = slot.is_hit();
            let prev_hit = self.prev_hit[slot_index];
            let was_suppressed = std::mem::take(&mut self.suppressed[slot_index]);

            let just_hit = cur_hit && !prev_hit;

            if just_hit && !was_suppressed {
                match cur_owner {
                    Some(owner) => despawning.push(owner),
                    None => eprintln!(
                        "writer: DespawnWatcher::poll: item slot {slot_index} was hit but has no resolvable owner -- not reporting"
                    ),
                }
            }

            self.prev_hit[slot_index] = cur_hit;
        }
        despawning
    }
}

#[derive(Default)]
pub struct ConsumeWatcher {
    prev_owner_kart_slot: [Option<u8>; wram::NUM_ITEM_SLOTS],
    suppressed: [bool; wram::NUM_ITEM_SLOTS],
}

impl ConsumeWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn suppress_slot(&mut self, slot_index: usize) {
        if slot_index < wram::NUM_ITEM_SLOTS {
            self.suppressed[slot_index] = true;
        }
    }

    pub fn poll(&mut self, ram: &[u8]) -> Vec<u8> {
        let mut consumed = Vec::new();
        for slot_index in 0..wram::NUM_ITEM_SLOTS {
            let Some(slot) = read_item_slot(ram, slot_index, "ConsumeWatcher::poll") else { continue };
            let cur_owner = slot.owner_kart_slot();

            let prev_owner = self.prev_owner_kart_slot[slot_index];
            let was_suppressed = std::mem::take(&mut self.suppressed[slot_index]);

            if let Some(owner) = prev_owner {
                if cur_owner.is_none() && !was_suppressed {
                    consumed.push(owner);
                }
            }

            self.prev_owner_kart_slot[slot_index] = cur_owner;
        }
        consumed
    }
}

fn read_mailbox_word(ram: &[u8]) -> Option<u16> {
    if wram::MAILBOX_ADDR + 1 >= ram.len() {
        eprintln!(
            "writer: read_mailbox_word: out of bounds (addr {:#x}, ram len {:#x})",
            wram::MAILBOX_ADDR,
            ram.len()
        );
        return None;
    }
    Some(u16::from_le_bytes([ram[wram::MAILBOX_ADDR], ram[wram::MAILBOX_ADDR + 1]]))
}

pub fn try_queue_item_throw(ram: &mut [u8], kart_slot: u8, item_type: u8, alt_direction: bool) -> bool {
    if kart_slot == wram::LOCAL_KART_SLOT || kart_slot >= wram::NUM_KART_SLOTS {
        eprintln!(
            "writer: try_queue_item_throw: kart_slot {kart_slot} invalid -- must be a REMOTE slot (1..{})",
            wram::NUM_KART_SLOTS
        );
        return false;
    }
    if !wram::is_valid_item_type(item_type) {
        eprintln!(
            "writer: try_queue_item_throw: item_type {item_type} is not a recognized Y-value (expected {}, {}, or {})",
            wram::ITEM_TYPE_BANANA,
            wram::ITEM_TYPE_RED_SHELL,
            wram::ITEM_TYPE_GREEN_SHELL
        );
        return false;
    }
    let Some(word) = read_mailbox_word(ram) else {
        return false;
    };
    if word & wram::ITEM_THROW_REQUEST_SLOT_MASK != 0 {
        return false;
    }

    let new_word = (word & wram::ITEM_THROW_REQUEST_PRESERVE_MASK)
        | ((kart_slot as u16) << wram::ITEM_THROW_REQUEST_SLOT_SHIFT)
        | ((item_type as u16) << wram::ITEM_THROW_REQUEST_TYPE_SHIFT)
        | if alt_direction { wram::ITEM_THROW_DIRECTION_BIT } else { 0 };
    let bytes = new_word.to_le_bytes();
    ram[wram::MAILBOX_ADDR] = bytes[0];
    ram[wram::MAILBOX_ADDR + 1] = bytes[1];
    true
}

#[derive(Default)]
pub struct LightningWatcher {
    prev_bit_set: bool,
}

impl LightningWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn poll(&mut self, ram: &[u8]) -> bool {
        const LIGHTNING_STRIKE_BIT: u8 = 0x08;
        let cur = ram.get(wram::LIGHTNING_SFX_FLAG_ADDR).copied().unwrap_or(0) & LIGHTNING_STRIKE_BIT != 0;
        let edge = cur && !self.prev_bit_set;
        self.prev_bit_set = cur;
        edge
    }
}

pub fn lightning_request_pending(ram: &[u8]) -> bool {
    read_mailbox_word(ram).is_some_and(|word| word & wram::LIGHTNING_REQUEST_BIT != 0)
}

pub fn try_queue_lightning_used(ram: &mut [u8], kart_slot: u8) -> bool {
    if kart_slot == wram::LOCAL_KART_SLOT || kart_slot >= wram::NUM_KART_SLOTS {
        eprintln!(
            "writer: try_queue_lightning_used: kart_slot {kart_slot} invalid -- must be a REMOTE slot (1..{})",
            wram::NUM_KART_SLOTS
        );
        return false;
    }
    let Some(word) = read_mailbox_word(ram) else {
        return false;
    };
    if word & wram::LIGHTNING_REQUEST_BIT != 0 {
        return false;
    }

    let new_word = (word & wram::LIGHTNING_REQUEST_PRESERVE_MASK)
        | wram::LIGHTNING_REQUEST_BIT
        | ((kart_slot as u16) << wram::LIGHTNING_SLOT_SHIFT);
    let bytes = new_word.to_le_bytes();
    ram[wram::MAILBOX_ADDR] = bytes[0];
    ram[wram::MAILBOX_ADDR + 1] = bytes[1];
    true
}

pub struct PristineTileMap {
    bytes: Box<[u8; wram::TILE_MAP_SIZE]>,
    captured: bool,
}

impl PristineTileMap {
    pub fn new() -> Self {
        Self { bytes: Box::new([0; wram::TILE_MAP_SIZE]), captured: false }
    }

    pub fn capture_once(&mut self, ram: &[u8]) {
        if self.captured {
            return;
        }
        let end = wram::TILE_MAP_BASE + wram::TILE_MAP_SIZE;
        if end > ram.len() {
            eprintln!("PristineTileMap::capture_once: tile map falls outside WRAM, skipping capture");
            return;
        }
        self.bytes.copy_from_slice(&ram[wram::TILE_MAP_BASE..end]);
        self.captured = true;
    }

    pub fn get(&self, tile_index: u16) -> u8 {
        self.bytes[tile_index as usize]
    }
}

impl Default for PristineTileMap {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CollectibleKind {
    Coin,
    ItemBox,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CollectibleTaken {
    pub tile_index: u16,
    pub kind: CollectibleKind,
    pub byte0: u8,
    pub byte1: Option<u8>,
}

#[derive(Default)]
pub struct CollectibleWatcher {
    prev_tile_index: Option<u16>,
    prev_tile_type: u8,
}

impl CollectibleWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn poll(&mut self, ram: &[u8], pristine: &PristineTileMap) -> Option<CollectibleTaken> {
        let base = wram::racer_offset(wram::LOCAL_KART_SLOT);
        let type_addr = base + wram::CURRENT_TILE_TYPE_OFFSET;
        let index_addr = base + wram::CURRENT_TILE_INDEX_OFFSET;
        if type_addr >= ram.len() || index_addr + 1 >= ram.len() {
            eprintln!(
                "CollectibleWatcher::poll: local racer's tile-type ({type_addr:#x}) or tile-index ({index_addr:#x}) falls outside WRAM, skipping"
            );
            return None;
        }

        let cur_type = ram[type_addr];
        let cur_index = u16::from_le_bytes([ram[index_addr], ram[index_addr + 1]]);

        let prev_index = self.prev_tile_index;
        let prev_type = self.prev_tile_type;
        self.prev_tile_index = Some(cur_index);
        self.prev_tile_type = cur_type;

        if prev_index != Some(cur_index) {
            return None;
        }

        let kind = if prev_type == wram::SURFACE_COIN && cur_type != wram::SURFACE_COIN {
            CollectibleKind::Coin
        } else if prev_type == wram::SURFACE_ITEM_PANEL_UNUSED && cur_type != wram::SURFACE_ITEM_PANEL_UNUSED {
            CollectibleKind::ItemBox
        } else {
            return None;
        };

        let (read_index, byte1_present) = match kind {
            CollectibleKind::Coin => (cur_index, false),
            CollectibleKind::ItemBox => {
                let old_byte = pristine.get(cur_index);
                let delta = wram::ITEM_BOX_VRAM_DELTA[(old_byte & 0x03) as usize];
                (cur_index.wrapping_add(delta as u16), true)
            }
        };

        let read_addr = wram::TILE_MAP_BASE + read_index as usize;
        if read_addr >= ram.len() {
            eprintln!(
                "CollectibleWatcher::poll: read_index {read_index} (from racer tile {cur_index}) out of range for tile map (addr {read_addr:#x} >= ram len {:#x}), dropping",
                ram.len()
            );
            return None;
        }
        let byte0 = ram[read_addr];
        let byte1 = if byte1_present {
            let below_addr = read_addr + wram::TILE_MAP_ROW_STRIDE;
            if below_addr < ram.len() {
                Some(ram[below_addr])
            } else {
                eprintln!(
                    "CollectibleWatcher::poll: item-box companion tile at {below_addr:#x} out of range, sending byte0 only"
                );
                None
            }
        } else {
            None
        };

        Some(CollectibleTaken { tile_index: cur_index, kind, byte0, byte1 })
    }
}

pub fn apply_collectible_tile_bytes(
    ram: &mut [u8],
    pristine: &PristineTileMap,
    tile_index: u16,
    kind: CollectibleKind,
    byte0: u8,
    byte1: Option<u8>,
) {
    let tile_addr = wram::TILE_MAP_BASE + tile_index as usize;
    if tile_addr >= ram.len() {
        eprintln!(
            "apply_collectible_tile_bytes: tile_index {tile_index} out of range for tile map (addr {tile_addr:#x} >= ram len {:#x}), dropping",
            ram.len()
        );
        return;
    }

    if kind == CollectibleKind::Coin {
        ram[tile_addr] = byte0;
        return;
    }

    let old_byte = pristine.get(tile_index);
    let delta = wram::ITEM_BOX_VRAM_DELTA[(old_byte & 0x03) as usize];
    let anchor = tile_index.wrapping_add(delta as u16);
    let bottom_byte = byte1.unwrap_or(byte0);

    let cells: [(u16, u8); 4] = [
        (anchor, byte0),
        (anchor.wrapping_add(1), byte0),
        (anchor.wrapping_add(wram::TILE_MAP_ROW_STRIDE as u16), bottom_byte),
        (anchor.wrapping_add(wram::TILE_MAP_ROW_STRIDE as u16 + 1), bottom_byte),
    ];
    for (idx, byte) in cells {
        let addr = wram::TILE_MAP_BASE + idx as usize;
        if addr < ram.len() {
            ram[addr] = byte;
        } else {
            eprintln!("apply_collectible_tile_bytes: item-box footprint cell {idx} (addr {addr:#x}) out of range, skipping that cell");
        }
    }
}

pub fn try_apply_collectible_vram_update(
    ram: &mut [u8],
    pristine: &PristineTileMap,
    tile_index: u16,
    kind: CollectibleKind,
    _byte0: u8,
) -> bool {
    if wram::VRAM_UPDATE_GROUP_ADDR + 1 >= ram.len() {
        eprintln!("try_apply_collectible_vram_update: VRAM update flag registers out of range, dropping");
        return true;
    }

    let flag_now = u16::from_le_bytes([ram[wram::VRAM_UPDATE_FLAG_ADDR], ram[wram::VRAM_UPDATE_FLAG_ADDR + 1]]);
    if flag_now != 0 {
        return false;
    }

    let (flag_val, group_val): (u16, u16) = match kind {
        CollectibleKind::Coin => (tile_index | 0x8000, 0),
        CollectibleKind::ItemBox => {
            let old_byte = pristine.get(tile_index);
            let delta = wram::ITEM_BOX_VRAM_DELTA[(old_byte & 0x03) as usize];
            (tile_index.wrapping_add(delta as u16), (old_byte & 0x0C) as u16)
        }
    };

    let flag_bytes = flag_val.to_le_bytes();
    ram[wram::VRAM_UPDATE_FLAG_ADDR] = flag_bytes[0];
    ram[wram::VRAM_UPDATE_FLAG_ADDR + 1] = flag_bytes[1];
    let group_bytes = group_val.to_le_bytes();
    ram[wram::VRAM_UPDATE_GROUP_ADDR] = group_bytes[0];
    ram[wram::VRAM_UPDATE_GROUP_ADDR + 1] = group_bytes[1];

    true
}