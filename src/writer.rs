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
    debug_assert!(
        track_index < wram::NUM_TRACKS,
        "track_index {track_index} out of range (0..{})",
        wram::NUM_TRACKS
    );
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
    EXCLUDED_RANGES.iter().any(|&(lo, hi)| offset >= lo && offset <= hi)
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

const POSITION_FIELD_RANGES: &[(u8, u8)] = &[
    (0x16, 0x21),
    (0x22, 0x27),
    (0x2A, 0x2B),
    (0x40, 0x41),
                   
];

fn is_position_field(offset: u8) -> bool {
    POSITION_FIELD_RANGES.iter().any(|&(lo, hi)| offset >= lo && offset <= hi)
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

fn remove_unoccupied_cpu(ram: &mut [u8], kart_slot: u8) {
    let base = wram::racer_offset(kart_slot);
    if base + 0xD4 > ram.len() {
        eprintln!("writer: remove_unoccupied_cpu: kart slot {kart_slot} struct falls outside WRAM, skipping");
        return;
    }
    let flag_bytes = REMOVED_FLAG_SET_1.to_le_bytes();
    ram[base + 0x10] = flag_bytes[0];
    ram[base + 0x11] = flag_bytes[1];
    let screen_bytes = REMOVED_SCREEN_NUMBER.to_le_bytes();
    ram[base + 0xD2] = screen_bytes[0];
    ram[base + 0xD3] = screen_bytes[1];
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

#[derive(Debug, Default)]
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

pub fn remove_unoccupied_cpus(ram: &mut [u8], occupied_kart_slots: &[u8]) {
    let mut occupied = [false; wram::NUM_KART_SLOTS as usize];
    for &kart_slot in occupied_kart_slots {
        if (kart_slot as usize) < occupied.len() {
            occupied[kart_slot as usize] = true;
        }
    }
    for kart_slot in 0..wram::NUM_KART_SLOTS {
        if !occupied[kart_slot as usize] {
            remove_unoccupied_cpu(ram, kart_slot);
        }
    }
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

const PACKET_TYPE_RACER: u8 = b'R';

const PACKET_HEADER_LEN: usize = 1 + 1 + 2 + 2;

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Default)]
pub struct SequenceGate {
    last_seen: std::collections::HashMap<wram::ServerSlot, u16>,
}

impl SequenceGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accept(&mut self, server_slot: wram::ServerSlot, sequence: u16) -> bool {
        match self.last_seen.get(&server_slot) {
            Some(&last) if sequence <= last => false,
            _ => {
                self.last_seen.insert(server_slot, sequence);
                true
            }
        }
    }
}

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};

pub struct RacerMirrorSocket {
    socket: UdpSocket,
    server_addr: SocketAddr,
    token: [u8; 8],
    
    peer_addrs: HashMap<wram::ServerSlot, SocketAddr>,
    sequence_gate: SequenceGate,
    
    next_sequence: u16,
    
    received_this_race: std::collections::HashSet<wram::ServerSlot>,
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
            peer_addrs: HashMap::new(),
            sequence_gate: SequenceGate::new(),
            next_sequence: 0,
            received_this_race: std::collections::HashSet::new(),
        })
    }

    pub fn reset_for_new_race(&mut self) {
        self.received_this_race.clear();
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
            let (len, from) = match self.socket.recv_from(&mut buf) {
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
            self.peer_addrs.insert(packet.server_slot, from);
            if !self.sequence_gate.accept(packet.server_slot, packet.sequence) {
                continue;
            }
            match slot_map.get(packet.server_slot) {
                Some(kart_slot) => {
                    if self.received_this_race.insert(packet.server_slot) {
                        
                        eprintln!(
                            "writer: first racer-mirror packet for server_slot {} -- Z-pin hide ends, applying real data",
                            packet.server_slot
                        );
                    }
                    apply_incoming_racer_data(ram, kart_slot, &packet.payload);
                }
                None => {
                    
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_bit_without_disturbing_others() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        ram[wram::MAILBOX_ADDR] = wram::ALL_READY_BIT;
        write_mailbox_bit(&mut ram, wram::CAN_ENTER_RACE_BIT, true);
        assert_eq!(ram[wram::MAILBOX_ADDR], wram::ALL_READY_BIT | wram::CAN_ENTER_RACE_BIT);
    }

    #[test]
    fn clears_bit_without_disturbing_others() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        ram[wram::MAILBOX_ADDR] = wram::ALL_READY_BIT | wram::CAN_ENTER_RACE_BIT;
        write_mailbox_bit(&mut ram, wram::CAN_ENTER_RACE_BIT, false);
        assert_eq!(ram[wram::MAILBOX_ADDR], wram::ALL_READY_BIT);
    }

    #[test]
    fn mailbox_write_out_of_bounds_does_not_panic() {
        let mut ram = vec![0u8; 4];
        write_mailbox_bit(&mut ram, wram::ALL_READY_BIT, true);
    }

    #[test]
    fn rng_write_is_little_endian() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        write_rng_seed(&mut ram, 0xBEEF);
        assert_eq!(ram[wram::RNG_ADDR], 0xEF);
        assert_eq!(ram[wram::RNG_ADDR + 1], 0xBE);
    }

    #[test]
    fn rng_write_out_of_bounds_does_not_panic() {
        let mut ram = vec![0u8; 4];
        write_rng_seed(&mut ram, 0x1234);
    }

    #[test]
    fn excluded_offsets_match_lua_ranges() {
        
        for &off in &[0x00u8, 0x13, 0x14, 0x15, 0x2C, 0x3F, 0x40, 0x41, 0x74, 0x7F, 0x98, 0x9F, 0xA4, 0xA5, 0xB6, 0xBD, 0xD2, 0xD3, 0xDE, 0xDF, 0xE6] {
            assert!(is_excluded(off), "offset {off:#x} should be excluded");
        }
        
        for &off in &[0x10u8.wrapping_add(0x10), 0x20, 0x50, 0x60, 0x90, 0xC0, 0xF0] {
            assert!(!is_excluded(off), "offset {off:#x} should NOT be excluded");
        }
    }

    #[test]
    fn apply_incoming_writes_non_excluded_bytes() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let payload = [0xAAu8; wram::RACER_STRUCT_SIZE];
        apply_incoming_racer_data(&mut ram, 1, &payload);
        let base = wram::racer_offset(1);
        
        assert_eq!(ram[base + 0x20], 0xAA);
    }

    #[test]
    fn apply_incoming_skips_excluded_bytes() {
        let mut ram = vec![0xFFu8; wram::WRAM_SIZE];
        let payload = [0xAAu8; wram::RACER_STRUCT_SIZE];
        apply_incoming_racer_data(&mut ram, 1, &payload);
        let base = wram::racer_offset(1);
        
        assert_eq!(ram[base + 0xD2], 0xFF);
    }

    #[test]
    fn apply_incoming_never_writes_kart_slot_zero_accidentally() {
        
        let mut ram = vec![0x55u8; wram::WRAM_SIZE];
        let payload = [0xAAu8; wram::RACER_STRUCT_SIZE];
        apply_incoming_racer_data(&mut ram, 1, &payload);
        let local_base = wram::racer_offset(wram::LOCAL_KART_SLOT);
        assert!(ram[local_base..local_base + wram::RACER_STRUCT_SIZE].iter().all(|&b| b == 0x55));
    }

    #[test]
    fn apply_incoming_rejects_wrong_size_payload() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let short_payload = [0xAAu8; 10];
        apply_incoming_racer_data(&mut ram, 1, &short_payload);
        let base = wram::racer_offset(1);
        assert!(ram[base..base + wram::RACER_STRUCT_SIZE].iter().all(|&b| b == 0));
    }

    #[test]
    fn extract_reads_local_racer_struct() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let base = wram::racer_offset(wram::LOCAL_KART_SLOT);
        ram[base] = 0x42;
        ram[base + 0xFF] = 0x99;
        let out = extract_local_racer_data(&ram);
        assert_eq!(out[0], 0x42);
        assert_eq!(out[0xFF], 0x99);
    }

    #[test]
    fn extract_does_not_exclude_anything() {
        
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let base = wram::racer_offset(wram::LOCAL_KART_SLOT);
        ram[base + 0xD2] = 0x7E;
        let out = extract_local_racer_data(&ram);
        assert_eq!(out[0xD2], 0x7E);
    }

    fn set_rank(ram: &mut [u8], kart_slot: u8, rank_1_indexed: u8) {
        let base = wram::racer_offset(kart_slot);
        let raw = ((rank_1_indexed - 1) as u16) * 2;
        let bytes = raw.to_le_bytes();
        ram[base + 0x40] = bytes[0];
        ram[base + 0x41] = bytes[1];
    }

    #[test]
    fn position_swap_moves_position_fields_not_identity_fields() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        
        set_rank(&mut ram, 0, 8);
        
        set_rank(&mut ram, 3, 2);

        let local_base = wram::racer_offset(0);
        let other_base = wram::racer_offset(3);

        ram[local_base + 0x18] = 0x11;
        ram[other_base + 0x18] = 0x22;
        
        ram[local_base + 0x12] = 0x04;
        ram[other_base + 0x12] = 0x0E;

        apply_position_to_slot(&mut ram, wram::LOCAL_KART_SLOT, 2);

        assert_eq!(ram[local_base + 0x18], 0x22);
        assert_eq!(ram[other_base + 0x18], 0x11);
        
        assert_eq!(u16::from_le_bytes([ram[local_base + 0x40], ram[local_base + 0x41]]), 2);
        
        assert_eq!(ram[local_base + 0x12], 0x04);
        assert_eq!(ram[other_base + 0x12], 0x0E);
    }

    #[test]
    fn position_swap_is_a_noop_when_target_already_holds_that_rank() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        set_rank(&mut ram, 0, 8);
        let local_base = wram::racer_offset(0);
        ram[local_base + 0x18] = 0x99;

        apply_position_to_slot(&mut ram, wram::LOCAL_KART_SLOT, 8);

        assert_eq!(ram[local_base + 0x18], 0x99, "no-op: nothing should have moved");
    }

    #[test]
    fn position_swap_missing_rank_does_not_panic_or_write() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let local_base = wram::racer_offset(0);
        ram[local_base + 0x18] = 0x55;

        apply_position_to_slot(&mut ram, wram::LOCAL_KART_SLOT, 3);

        assert_eq!(ram[local_base + 0x18], 0x55, "should be left untouched");
    }

    #[test]
    fn position_swap_out_of_range_does_not_panic() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        apply_position_to_slot(&mut ram, wram::LOCAL_KART_SLOT, 0);
        apply_position_to_slot(&mut ram, wram::LOCAL_KART_SLOT, 9);
    }

    #[test]
    fn remove_unoccupied_cpu_stamps_the_captured_signature() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let base = wram::racer_offset(5);
        
        ram[base + 0x10] = 0x00;
        ram[base + 0x11] = 0x20;
        ram[base + 0xD2] = 0x00;
        ram[base + 0xD3] = 0x00;

        remove_unoccupied_cpu(&mut ram, 5);

        assert_eq!(u16::from_le_bytes([ram[base + 0x10], ram[base + 0x11]]), 0x0000);
        assert_eq!(u16::from_le_bytes([ram[base + 0xD2], ram[base + 0xD3]]), 0xFFFF);
    }

    #[test]
    fn startup_placement_runs_all_three_steps() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        
        for kart_slot in 0..wram::NUM_KART_SLOTS {
            let rank = if kart_slot == 0 { 8 } else { kart_slot };
            set_rank(&mut ram, kart_slot, rank);
            
            ram[wram::racer_offset(kart_slot) + 0x11] = 0x20;
        }

        let others = [(2u8, 1u8)];
        run_startup_placement(&mut ram, 3, &others);

        let local_base = wram::racer_offset(0);
        assert_eq!(u16::from_le_bytes([ram[local_base + 0x40], ram[local_base + 0x41]]), 4);

        let other_base = wram::racer_offset(2);
        assert_eq!(u16::from_le_bytes([ram[other_base + 0x40], ram[other_base + 0x41]]), 0);

        for kart_slot in 0..wram::NUM_KART_SLOTS {
            let base = wram::racer_offset(kart_slot);
            let screen = u16::from_le_bytes([ram[base + 0xD2], ram[base + 0xD3]]);
            if kart_slot == 0 || kart_slot == 2 {
                assert_ne!(screen, 0xFFFF, "kart slot {kart_slot} is occupied and should NOT be removed");
            } else {
                assert_eq!(screen, 0xFFFF, "kart slot {kart_slot} is unoccupied and should be removed");
            }
        }
    }

    #[test]
    fn remove_unoccupied_cpus_re_stamps_a_cpu_that_drifted_back_to_occupied() {
        
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let base = wram::racer_offset(4);
        ram[base + 0x10] = 0x00;
        ram[base + 0x11] = 0x20;
        ram[base + 0xD2] = 0x00;
        ram[base + 0xD3] = 0x00;

        let occupied_kart_slots = [wram::LOCAL_KART_SLOT];
        remove_unoccupied_cpus(&mut ram, &occupied_kart_slots);

        assert_eq!(u16::from_le_bytes([ram[base + 0x10], ram[base + 0x11]]), 0x0000);
        assert_eq!(u16::from_le_bytes([ram[base + 0xD2], ram[base + 0xD3]]), 0xFFFF);
    }

    #[test]
    fn remove_unoccupied_cpus_leaves_occupied_slots_alone() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let local_base = wram::racer_offset(wram::LOCAL_KART_SLOT);
        let remote_base = wram::racer_offset(3);
        ram[local_base + 0x11] = 0x80;
        ram[remote_base + 0x11] = 0x80;
        ram[local_base + 0xD2] = 0x00;
        ram[remote_base + 0xD2] = 0x02;

        remove_unoccupied_cpus(&mut ram, &[wram::LOCAL_KART_SLOT, 3]);

        assert_ne!(u16::from_le_bytes([ram[local_base + 0xD2], ram[local_base + 0xD3]]), 0xFFFF);
        assert_ne!(u16::from_le_bytes([ram[remote_base + 0xD2], ram[remote_base + 0xD3]]), 0xFFFF);
    }

    #[test]
    fn remove_unoccupied_cpus_is_idempotent() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let occupied_kart_slots = [wram::LOCAL_KART_SLOT];
        remove_unoccupied_cpus(&mut ram, &occupied_kart_slots);
        let after_first = ram.clone();
        remove_unoccupied_cpus(&mut ram, &occupied_kart_slots);
        assert_eq!(ram, after_first, "calling it again with the same occupied set should change nothing further");
    }

    #[test]
    fn non_collision_pins_z_to_max_for_cpu_slots_only() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT, 2];

        apply_non_collision_to_cpus(&mut ram, &human_kart_slots);

        for kart_slot in 0..wram::NUM_KART_SLOTS {
            let base = wram::racer_offset(kart_slot);
            let z = u16::from_le_bytes([
                ram[base + wram::Z_POSITION_OFFSET],
                ram[base + wram::Z_POSITION_OFFSET + 1],
            ]);
            if human_kart_slots.contains(&kart_slot) {
                assert_eq!(z, 0, "human slot {kart_slot} should be untouched");
            } else {
                assert_eq!(z, wram::Z_POSITION_MAX, "CPU slot {kart_slot} should be pinned to max Z");
            }
        }
    }

    fn set_current_lap_byte(ram: &mut [u8], kart_slot: u8, value: u8) {
        let base = wram::racer_offset(kart_slot);
        ram[base + wram::CURRENT_LAP_OFFSET] = value;
    }

    #[test]
    fn auto_finish_does_nothing_until_all_humans_finish() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT, 2];
        set_current_lap_byte(&mut ram, wram::LOCAL_KART_SLOT, wram::FINISHED_LAP_VALUE);
        set_current_lap_byte(&mut ram, 2, 0x82);
        set_current_lap_byte(&mut ram, 5, 0x81);

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);

        assert_eq!(
            ram[wram::racer_offset(5) + wram::CURRENT_LAP_OFFSET],
            0x81,
            "should not touch any CPU while a human is still racing"
        );
        assert_eq!(
            u16::from_le_bytes([ram[FINISH_QUEUE_COUNT_ADDR], ram[FINISH_QUEUE_COUNT_ADDR + 1]]),
            0,
            "finish queue should stay empty while a human is still racing"
        );
    }

    #[test]
    fn auto_finish_sets_0x85_on_every_cpu_once_all_humans_are_done() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT, 2];
        for &slot in &human_kart_slots {
            set_current_lap_byte(&mut ram, slot, wram::FINISHED_LAP_VALUE);
        }
        for kart_slot in 0..wram::NUM_KART_SLOTS {
            if human_kart_slots.contains(&kart_slot) {
                continue;
            }
            set_current_lap_byte(&mut ram, kart_slot, 0x82);
            
            ram[wram::racer_offset(kart_slot) + CHECKPOINT_OFFSET] = 0x07;
        }

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);

        for kart_slot in 0..wram::NUM_KART_SLOTS {
            let value = ram[wram::racer_offset(kart_slot) + wram::CURRENT_LAP_OFFSET];
            assert_eq!(value, wram::FINISHED_LAP_VALUE, "kart slot {kart_slot} should read 0x85 after auto-finish");
        }
    }

    #[test]
    fn auto_finish_zeroes_checkpoint_byte_alongside_lap_byte() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT];
        set_current_lap_byte(&mut ram, wram::LOCAL_KART_SLOT, wram::FINISHED_LAP_VALUE);
        let cpu_base = wram::racer_offset(1);
        ram[cpu_base + CHECKPOINT_OFFSET] = 0x0A;

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);

        assert_eq!(ram[cpu_base + CHECKPOINT_OFFSET], 0x00, "checkpoint must be zeroed alongside the lap byte");
    }

    #[test]
    fn auto_finish_enqueues_every_force_finished_cpu_onto_the_finish_queue() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        
        let human_kart_slots = [wram::LOCAL_KART_SLOT];
        set_current_lap_byte(&mut ram, wram::LOCAL_KART_SLOT, wram::FINISHED_LAP_VALUE);

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);

        let count = u16::from_le_bytes([ram[FINISH_QUEUE_COUNT_ADDR], ram[FINISH_QUEUE_COUNT_ADDR + 1]]);
        assert_eq!(count, 14, "7 force-finished CPUs * 2 bytes/entry = 14");

        let mut seen: Vec<u16> = Vec::new();
        for i in 0..(count as usize / 2) {
            let slot_addr = FINISH_QUEUE_BASE + i * 2;
            let entry = u16::from_le_bytes([ram[slot_addr], ram[slot_addr + 1]]);
            seen.push(entry);
        }
        seen.sort_unstable();
        let mut expected: Vec<u16> = (1..wram::NUM_KART_SLOTS).map(|slot| wram::racer_offset(slot) as u16).collect();
        expected.sort_unstable();
        assert_eq!(seen, expected, "every non-human kart slot's struct offset should be queued exactly once");
    }

    #[test]
    fn auto_finish_does_not_double_enqueue_already_finished_cpus() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT, 2];
        for &slot in &human_kart_slots {
            set_current_lap_byte(&mut ram, slot, wram::FINISHED_LAP_VALUE);
        }
        
        set_current_lap_byte(&mut ram, 1, wram::FINISHED_LAP_VALUE);

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);

        let count = u16::from_le_bytes([ram[FINISH_QUEUE_COUNT_ADDR], ram[FINISH_QUEUE_COUNT_ADDR + 1]]);
        
        assert_eq!(count, 10, "already-finished kart slot 1 should not be re-enqueued");
        let queued_offsets: Vec<u16> = (0..(count as usize / 2))
            .map(|i| {
                let slot_addr = FINISH_QUEUE_BASE + i * 2;
                u16::from_le_bytes([ram[slot_addr], ram[slot_addr + 1]])
            })
            .collect();
        assert!(
            !queued_offsets.contains(&(wram::racer_offset(1) as u16)),
            "kart slot 1 was already finished and must not appear in the queue"
        );
    }

    #[test]
    fn auto_finish_is_idempotent_across_polls() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT];
        set_current_lap_byte(&mut ram, wram::LOCAL_KART_SLOT, wram::FINISHED_LAP_VALUE);

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);
        let after_first = ram.clone();
        finisher.poll(&mut ram, &human_kart_slots);

        assert_eq!(ram, after_first, "second poll after done should not touch WRAM again");
    }

    #[test]
    fn force_finish_leaves_lap_byte_untouched_when_queue_is_full() {
        
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let full_count = (FINISH_QUEUE_CAPACITY * 2).to_le_bytes();
        ram[FINISH_QUEUE_COUNT_ADDR] = full_count[0];
        ram[FINISH_QUEUE_COUNT_ADDR + 1] = full_count[1];

        let succeeded = force_finish_cpu(&mut ram, 5);

        assert!(!succeeded, "enqueue should fail when the queue is already full");
        assert_eq!(
            ram[wram::racer_offset(5) + wram::CURRENT_LAP_OFFSET],
            0,
            "lap byte must stay untouched so has_finished() doesn't lie about this CPU"
        );
        assert!(
            !has_finished(&ram, 5),
            "a CPU that failed to enqueue must not look finished to has_finished()"
        );
    }

    #[test]
    fn auto_finish_retries_a_straggler_once_queue_capacity_frees_up() {
        
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT];
        set_current_lap_byte(&mut ram, wram::LOCAL_KART_SLOT, wram::FINISHED_LAP_VALUE);

        let full_count = (FINISH_QUEUE_CAPACITY * 2).to_le_bytes();
        ram[FINISH_QUEUE_COUNT_ADDR] = full_count[0];
        ram[FINISH_QUEUE_COUNT_ADDR + 1] = full_count[1];

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);

        assert!(!finisher.done, "must not report done when every CPU failed to enqueue");
        for kart_slot in 1..wram::NUM_KART_SLOTS {
            assert_ne!(
                ram[wram::racer_offset(kart_slot) + wram::CURRENT_LAP_OFFSET],
                wram::FINISHED_LAP_VALUE,
                "kart slot {kart_slot} should not look finished while the queue was full"
            );
        }

        ram[FINISH_QUEUE_COUNT_ADDR] = 0;
        ram[FINISH_QUEUE_COUNT_ADDR + 1] = 0;

        finisher.poll(&mut ram, &human_kart_slots);

        assert!(finisher.done, "should succeed and report done once queue capacity is available");
        for kart_slot in 1..wram::NUM_KART_SLOTS {
            assert_eq!(
                ram[wram::racer_offset(kart_slot) + wram::CURRENT_LAP_OFFSET],
                wram::FINISHED_LAP_VALUE,
                "kart slot {kart_slot} should be force-finished on retry"
            );
        }
    }

    #[test]
    fn auto_finish_reset_allows_a_new_race_to_trigger_again() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let human_kart_slots = [wram::LOCAL_KART_SLOT];
        set_current_lap_byte(&mut ram, wram::LOCAL_KART_SLOT, wram::FINISHED_LAP_VALUE);

        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &human_kart_slots);
        assert!(finisher.done);

        finisher.reset();
        assert!(!finisher.done);
    }

    #[test]
    fn no_humans_tracked_never_triggers_auto_finish() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        
        let mut finisher = AutoFinisher::new();
        finisher.poll(&mut ram, &[]);
        for kart_slot in 0..wram::NUM_KART_SLOTS {
            assert_eq!(ram[wram::racer_offset(kart_slot) + wram::CURRENT_LAP_OFFSET], 0);
        }
    }

    #[test]
    fn hide_pending_remote_slots_z_pins_unreceived_slots_only() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let kart_slot = 3u8;
        let base = wram::racer_offset(kart_slot);
        ram[base + 0x11] = 0x80;

        let mut mirror = RacerMirrorSocket::new("127.0.0.1:0", "127.0.0.1:0", [0u8; 8]).unwrap();
        mirror.hide_pending_remote_slots(&mut ram, &[(5u8, kart_slot)]);

        assert_eq!(
            u16::from_le_bytes([ram[base + wram::Z_POSITION_OFFSET], ram[base + wram::Z_POSITION_OFFSET + 1]]),
            wram::Z_POSITION_MAX
        );
        
        assert_eq!(ram[base + 0x11], 0x80);
        assert_ne!(u16::from_le_bytes([ram[base + 0xD2], ram[base + 0xD3]]), REMOVED_SCREEN_NUMBER);
    }

    #[test]
    fn hide_pending_remote_slots_skips_slots_already_received_this_race() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let kart_slot = 4u8;
        let base = wram::racer_offset(kart_slot);
        let z_bytes = 0x1234u16.to_le_bytes();
        ram[base + wram::Z_POSITION_OFFSET] = z_bytes[0];
        ram[base + wram::Z_POSITION_OFFSET + 1] = z_bytes[1];

        let mut mirror = RacerMirrorSocket::new("127.0.0.1:0", "127.0.0.1:0", [0u8; 8]).unwrap();
        mirror.received_this_race.insert(7u8);
        mirror.hide_pending_remote_slots(&mut ram, &[(7u8, kart_slot)]);

        assert_eq!(
            u16::from_le_bytes([ram[base + wram::Z_POSITION_OFFSET], ram[base + wram::Z_POSITION_OFFSET + 1]]),
            0x1234
        );
    }

    #[test]
    fn poll_and_apply_marks_received_and_lets_real_data_override_the_z_pin() {
        let mut ram = vec![0u8; wram::WRAM_SIZE];
        let mut slot_map = wram::SlotMap::new();
        let kart_slot = slot_map.assign(5).unwrap();
        let base = wram::racer_offset(kart_slot);
        ram[base + 0x11] = 0x80;

        let mut mirror = RacerMirrorSocket::new("127.0.0.1:0", "127.0.0.1:0", [0u8; 8]).unwrap();
        mirror.hide_pending_remote_slots(&mut ram, &[(5u8, kart_slot)]);
        assert_eq!(
            u16::from_le_bytes([ram[base + wram::Z_POSITION_OFFSET], ram[base + wram::Z_POSITION_OFFSET + 1]]),
            wram::Z_POSITION_MAX,
            "sanity: should start Z-pinned"
        );

        let mut payload = [0x99u8; wram::RACER_STRUCT_SIZE];
        
        let real_z = 0x0042u16.to_le_bytes();
        payload[wram::Z_POSITION_OFFSET] = real_z[0];
        payload[wram::Z_POSITION_OFFSET + 1] = real_z[1];
        let packet = encode_packet(5, 1, &payload);
        mirror.socket.send_to(&packet, mirror.socket.local_addr().unwrap()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        mirror.poll_and_apply(&mut ram, &slot_map, 0);

        assert_eq!(
            u16::from_le_bytes([ram[base + wram::Z_POSITION_OFFSET], ram[base + wram::Z_POSITION_OFFSET + 1]]),
            0x0042,
            "real incoming Z should have overridden the pin, no separate restore needed"
        );
        assert_eq!(ram[base + 0x11], 0x80, "flag byte was never touched, so it's untouched still");
        assert!(mirror.received_this_race.contains(&5u8));

        mirror.hide_pending_remote_slots(&mut ram, &[(5u8, kart_slot)]);
        assert_eq!(
            u16::from_le_bytes([ram[base + wram::Z_POSITION_OFFSET], ram[base + wram::Z_POSITION_OFFSET + 1]]),
            0x0042,
            "already-received slot's real Z must not get re-pinned"
        );
    }

    #[test]
    fn packet_roundtrips() {
        let payload = [0x11u8; wram::RACER_STRUCT_SIZE];
        let bytes = encode_packet(3, 42, &payload);
        let decoded = decode_packet(&bytes).expect("should decode");
        assert_eq!(decoded.server_slot, 3);
        assert_eq!(decoded.sequence, 42);
        assert_eq!(decoded.payload, payload.to_vec());
    }

    #[test]
    fn decode_rejects_too_short() {
        assert!(decode_packet(&[b'R', 1, 0]).is_none());
    }

    #[test]
    fn decode_rejects_wrong_type() {
        let mut bytes = encode_packet(0, 0, &[1, 2, 3]);
        bytes[0] = b'X';
        assert!(decode_packet(&bytes).is_none());
    }

    #[test]
    fn decode_rejects_length_mismatch() {
        let mut bytes = encode_packet(0, 0, &[1, 2, 3]);
        
        bytes[4] = 0;
        bytes[5] = 99;
        assert!(decode_packet(&bytes).is_none());
    }

    #[test]
    fn sequence_gate_accepts_first_packet_from_a_sender() {
        let mut gate = SequenceGate::new();
        assert!(gate.accept(3, 1));
    }

    #[test]
    fn sequence_gate_accepts_strictly_increasing() {
        let mut gate = SequenceGate::new();
        assert!(gate.accept(3, 1));
        assert!(gate.accept(3, 2));
        assert!(gate.accept(3, 10));
    }

    #[test]
    fn sequence_gate_rejects_stale_or_duplicate() {
        let mut gate = SequenceGate::new();
        assert!(gate.accept(3, 10));
        assert!(!gate.accept(3, 10));
        assert!(!gate.accept(3, 5));
    }

    #[test]
    fn sequence_gate_tracks_senders_independently() {
        
        let mut gate = SequenceGate::new();
        assert!(gate.accept(1, 100));
        
        assert!(gate.accept(2, 1));
        assert!(gate.accept(2, 2));
        assert!(!gate.accept(1, 50));
    }
}