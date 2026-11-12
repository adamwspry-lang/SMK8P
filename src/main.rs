mod wram;
mod writer;
mod net;
mod lobby;
mod audio;
mod settings;

use audio::AudioOutput;
use eframe::egui;
use gilrs::Gilrs;
use lobby::LobbyApp;
use net::ServerEvent;
use retro_rs::{Buttons, Emulator};
use settings::{Settings, SettingsWindow};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::{Path, PathBuf};

const WIDTH: usize = 256;
const HEIGHT: usize = 224;

const KNOWN_GOOD_SHA1: &str = "47e103d8398cf5b7cbb42b95df3a3c270691163b";

const ROM_LOCATION_FILE: &str = "roms/rom_location.txt";

const PATCHED_ROM_PATH: &str = "roms/smk_patched.sfc";

const PATCHES_DIR: &str = "patches";

const ORDERED_PATCHES: &[&str] = &[
    "infinite_gp.ips",
    "no_rank_out.ips",
    "no_retry_button.ips",
    "wait_for_racers.ips",
];

const DEFAULT_SERVER_ADDR: &str = "2.29.45.28:9000";
const LOCAL_SERVER_ADDR: &str = "127.0.0.1:9000";

const UDP_BIND_ADDR: &str = "0.0.0.0:9001";
const DEFAULT_UDP_BROADCAST_ADDR: &str = "2.29.45.28:9001";

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon_256.png");

pub(crate) fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .expect("failed to get current exe path")
        .parent()
        .expect("exe has no parent dir")
        .to_path_buf()
}

fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_BYTES).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn sha1_hex(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn locate_vanilla_rom() -> PathBuf {
    let rom_location_file = exe_dir().join(ROM_LOCATION_FILE);

    if let Ok(remembered) = fs::read_to_string(&rom_location_file) {
        let remembered_path = PathBuf::from(remembered.trim());
        if remembered_path.exists() {
            match sha1_hex(&remembered_path) {
                Ok(hash) => {
                    if !KNOWN_GOOD_SHA1.is_empty() && hash != KNOWN_GOOD_SHA1 {
                        eprintln!(
                            "REJECTED: ROM at {remembered_path:?} does not match the known-good hash.\n  expected: {KNOWN_GOOD_SHA1}\n  found:    {hash}\nThis is not the correct ROM -- could be a different region (PAL/NTSC-J), a bad dump, or something that isn't the real ROM at all. Refusing to load it.\nDelete or fix {rom_location_file:?} and re-run to select a different file."
                        );
                        std::process::exit(1);
                    }
                    return remembered_path;
                }
                Err(e) => {
                    eprintln!("Could not hash remembered ROM at {remembered_path:?}: {e}. Re-prompting.");
                }
            }
        } else {
            println!("Previously selected ROM at {remembered_path:?} is no longer there.");
        }
    }

    println!("Please select your vanilla Super Mario Kart (USA) ROM file.");

    let picked = rfd::FileDialog::new()
        .add_filter("SNES ROM", &["sfc", "smc"])
        .set_title("Locate your Super Mario Kart (USA) ROM")
        .pick_file();

    let picked = picked.unwrap_or_else(|| {
        eprintln!("No file selected. Exiting.");
        std::process::exit(1);
    });

    match sha1_hex(&picked) {
        Ok(hash) => {
            if KNOWN_GOOD_SHA1.is_empty() {
                eprintln!(
                    "NOTE: KNOWN_GOOD_SHA1 is not set in main.rs, so this ROM's hash was NOT verified.\nIts SHA-1 is: {hash}\nIf this is a known-good dump, paste that into KNOWN_GOOD_SHA1 now so future runs verify it."
                );
            } else if hash != KNOWN_GOOD_SHA1 {
                eprintln!(
                    "REJECTED: selected ROM does not match the known-good hash.\n  expected: {KNOWN_GOOD_SHA1}\n  found:    {hash}\nThis is not the correct ROM -- could be a different region (PAL/NTSC-J), a bad dump, or something that isn't the real ROM at all. Refusing to load it."
                );
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Could not hash selected ROM: {e}. Refusing to load an unverifiable file.");
            std::process::exit(1);
        }
    }

    if let Some(parent) = rom_location_file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&rom_location_file, picked.to_string_lossy().as_bytes()) {
        eprintln!("Could not remember ROM location ({rom_location_file:?}): {e}. You'll be asked again next run.");
    }

    picked
}
struct IpsRecord {
    offset: usize,
    data: Vec<u8>,
}

fn parse_ips(patch_bytes: &[u8]) -> Result<Vec<IpsRecord>, String> {
    if patch_bytes.len() < 5 || &patch_bytes[0..5] != b"PATCH" {
        return Err("not a valid IPS file (missing PATCH header)".into());
    }
    let mut pos = 5;
    let mut records = Vec::new();
    loop {
        if pos + 3 > patch_bytes.len() {
            return Err("truncated IPS file".into());
        }
        if &patch_bytes[pos..pos + 3] == b"EOF" {
            break;
        }
        let offset = ((patch_bytes[pos] as usize) << 16)
            | ((patch_bytes[pos + 1] as usize) << 8)
            | (patch_bytes[pos + 2] as usize);
        pos += 3;

        if pos + 2 > patch_bytes.len() {
            return Err("truncated IPS file (cut off mid-record length)".into());
        }
        let len = ((patch_bytes[pos] as usize) << 8) | (patch_bytes[pos + 1] as usize);
        pos += 2;

        if len == 0 {
            if pos + 3 > patch_bytes.len() {
                return Err("truncated IPS file (cut off mid-RLE-record)".into());
            }
            let run_len = ((patch_bytes[pos] as usize) << 8) | (patch_bytes[pos + 1] as usize);
            pos += 2;
            let value = patch_bytes[pos];
            pos += 1;
            records.push(IpsRecord { offset, data: vec![value; run_len] });
        } else {
            if pos + len > patch_bytes.len() {
                return Err("truncated IPS file (record data cut off)".into());
            }
            records.push(IpsRecord { offset, data: patch_bytes[pos..pos + len].to_vec() });
            pos += len;
        }
    }
    Ok(records)
}

fn is_patch_applied(rom: &[u8], records: &[IpsRecord]) -> bool {
    records.iter().all(|r| {
        let end = r.offset + r.data.len();
        end <= rom.len() && rom[r.offset..end] == r.data[..]
    })
}

fn apply_records(rom: &mut Vec<u8>, records: &[IpsRecord]) {
    for r in records {
        let end = r.offset + r.data.len();
        if end > rom.len() {
            rom.resize(end, 0);
        }
        rom[r.offset..end].copy_from_slice(&r.data);
    }
}

fn discover_patches() -> Vec<PathBuf> {
    let patches_dir = exe_dir().join(PATCHES_DIR);

    let mut patches: Vec<PathBuf> =
        ORDERED_PATCHES.iter().map(|name| patches_dir.join(name)).collect();

    let mut extra: Vec<PathBuf> = Vec::new();
    match fs::read_dir(&patches_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ips") {
                    continue;
                }
                let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if ORDERED_PATCHES.contains(&file_name) {
                    continue;
                }
                extra.push(path);
            }
        }
        Err(e) => {
            eprintln!("Could not read patches directory {patches_dir:?}: {e}");
        }
    }
    extra.sort();
    for path in &extra {
        eprintln!(
            "NOTE: found {path:?} in {patches_dir:?} but it isn't listed in ORDERED_PATCHES -- \
             applying it last, after every pinned patch. Add its filename to ORDERED_PATCHES in \
             main.rs if it needs to run in a specific position relative to the others."
        );
    }
    patches.extend(extra);
    patches
}
fn build_patched_rom(vanilla_path: &Path) -> PathBuf {
    let out_path = exe_dir().join(PATCHED_ROM_PATH);

    let mut rom = match fs::read(&out_path) {
        Ok(bytes) => bytes,
        Err(_) => fs::read(vanilla_path).expect("failed to read vanilla ROM"),
    };

    for patch_path in discover_patches() {
        let patch_bytes = match fs::read(&patch_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Could not read IPS patch {patch_path:?}: {e}. Skipping it.");
                continue;
            }
        };
        let records = match parse_ips(&patch_bytes) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse {patch_path:?}: {e}. Skipping it.");
                continue;
            }
        };
        if is_patch_applied(&rom, &records) {
            println!("Already applied: {patch_path:?}");
        } else {
            apply_records(&mut rom, &records);
            println!("Applied patch: {patch_path:?}");
        }
    }

    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&out_path, &rom).expect("failed to write patched ROM");
    out_path
}
const TARGET_FPS: f64 = 60.098_812;

const RACE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(220);

struct GameFlowWatcher {
    prev_gamemode: u8,
    
    prev_countdown_hold: u8,
    
    prev_current_lap: u8,
    
    prev_lakitu_script_ptr: u16,
    
    has_raced: bool,
    
    has_readied_this_select: bool,
    
    pub countdown_hold_fired: bool,
    
    pub race_started_fired: bool,
    
    pub entered_race_edge: bool,
    
    pub should_broadcast: bool,
}

impl GameFlowWatcher {
    fn new() -> Self {
        Self {
            prev_gamemode: wram::GAMEMODE_NONE,
            prev_countdown_hold: 0,
            prev_current_lap: 0,
            
            prev_lakitu_script_ptr: 0,
            has_raced: false,
            has_readied_this_select: false,
            countdown_hold_fired: false,
            race_started_fired: false,
            entered_race_edge: false,
            should_broadcast: true,
        }
    }

    fn poll(
        &mut self,
        ram: &[u8],
        is_host: bool,
        joined_this_frame: bool,
        room_racing: bool,
        start_track: u8,
    ) -> Vec<net::Request> {
        let mut out = Vec::new();
        self.entered_race_edge = false;
        self.race_started_fired = false;

        let local_lap_addr = wram::racer_offset(wram::LOCAL_KART_SLOT) + wram::CURRENT_LAP_OFFSET;
        
        let local_rank_addr = wram::racer_offset(wram::LOCAL_KART_SLOT) + wram::CURRENT_RANK_OFFSET;
        
        if ram.len() <= wram::GAMEMODE_ADDR
            || ram.len() <= wram::COUNTDOWN_HOLD_ADDR
            || ram.len() <= wram::READY_SIGNAL_ADDR
            || ram.len() <= local_lap_addr
            || ram.len() <= wram::LAKITU_SCRIPT_PTR_ADDR + 1
            || ram.len() <= local_rank_addr + 1
        {
            eprintln!(
                "GameFlowWatcher::poll: WRAM slice too small (len {:#x}) to read gamemode ({:#x}), countdown-hold ack ({:#x}), ready signal ({:#x}), local current-lap ({:#x}), local current-rank ({:#x}), or Lakitu script pointer ({:#x}) -- core may not have WRAM mapped yet",
                ram.len(),
                wram::GAMEMODE_ADDR,
                wram::COUNTDOWN_HOLD_ADDR,
                wram::READY_SIGNAL_ADDR,
                local_lap_addr,
                local_rank_addr,
                wram::LAKITU_SCRIPT_PTR_ADDR
            );
            return out;
        }

        let gm = ram[wram::GAMEMODE_ADDR];
        let _ = joined_this_frame;

        if gm == wram::GAMEMODE_TRACK_SELECT && !room_racing && !self.has_readied_this_select {
            out.push(net::Request::Ready);
            self.has_readied_this_select = true;
        } else if gm != wram::GAMEMODE_TRACK_SELECT {
            self.has_readied_this_select = false;
        }

        if gm != wram::GAMEMODE_NONE && gm != self.prev_gamemode {
            match gm {
                wram::GAMEMODE_IN_RACE => {
                    self.has_raced = true;
                    self.entered_race_edge = true;
                    
                    self.prev_current_lap = 0xFF;
                    
                    self.prev_lakitu_script_ptr = 0;
                }
                wram::GAMEMODE_TITLE => {
                    if self.has_raced {
                        out.push(net::Request::LeaveRoom);
                    }
                    
                    self.has_raced = false;
                }
                _ => {}
            }
            self.prev_gamemode = gm;
        }

        let countdown_hold = ram[wram::COUNTDOWN_HOLD_ADDR];
        self.countdown_hold_fired = countdown_hold != 0 && self.prev_countdown_hold == 0;
        self.prev_countdown_hold = countdown_hold;

        let lakitu_ptr = u16::from_le_bytes([
            ram[wram::LAKITU_SCRIPT_PTR_ADDR],
            ram[wram::LAKITU_SCRIPT_PTR_ADDR + 1],
        ]);
        if gm == wram::GAMEMODE_IN_RACE
            && lakitu_ptr == wram::LAKITU_PARKED_VALUE
            && self.prev_lakitu_script_ptr != wram::LAKITU_PARKED_VALUE
        {
            eprintln!("Lakitu-parked edge fired ($7E:1C04 == {:#06x}) -- sending Ready", lakitu_ptr);
            out.push(net::Request::Ready);
        }
        self.prev_lakitu_script_ptr = lakitu_ptr;

        let current_lap = ram[local_lap_addr];

        self.race_started_fired =
            current_lap == wram::RACE_STARTED_LAP_VALUE && self.prev_current_lap != wram::RACE_STARTED_LAP_VALUE;

        let finished_edge =
            current_lap == wram::FINISHED_LAP_VALUE && self.prev_current_lap != wram::FINISHED_LAP_VALUE;
        if finished_edge {
            
            let rank_raw = u16::from_le_bytes([ram[local_rank_addr], ram[local_rank_addr + 1]]);
            let position = (rank_raw / 2 + 1).clamp(1, wram::NUM_KART_SLOTS as u16) as u8;
            eprintln!(
                "Finished edge fired (lap byte == {current_lap:#04x}) -- reporting position {position} (raw rank {rank_raw:#06x})"
            );
            out.push(net::Request::Finished { position: position.into() });
        }

        if gm == wram::GAMEMODE_IN_RACE
            && current_lap == wram::LAP_NOT_STARTED_VALUE
            && self.prev_current_lap != wram::LAP_NOT_STARTED_VALUE
        {
            eprintln!("Lap-not-started edge fired (lap byte == 0x7F)");
            if is_host {
                eprintln!("  -- host, sending StartRoom (start_track = {start_track})");
                out.push(net::Request::StartRoom { start_track: start_track as i32 });
            }
        }

        if current_lap == wram::FINISHED_LAP_VALUE && self.prev_current_lap == wram::FINISHED_LAP_VALUE {
            self.should_broadcast = false;
        } else if current_lap == wram::LAP_NOT_STARTED_VALUE {
            self.should_broadcast = true;
        }

        self.prev_current_lap = current_lap;

        let ready_signal = ram[wram::READY_SIGNAL_ADDR];
        let _ = ready_signal;

        out
    }
}

struct AppState {
    emu: Emulator,
    
    framebuffer_argb: Vec<u32>,
    framebuffer_rgba8: Vec<u8>,
    frame_dims: (usize, usize),
    texture: Option<egui::TextureHandle>,
    lobby: LobbyApp,
    
    audio: Option<AudioOutput>,

    last_tick: std::time::Instant,
    accumulator: std::time::Duration,

    settings: Settings,
    settings_window: SettingsWindow,
    
    gilrs: Option<Gilrs>,

    fps_frame_count: u32,
    fps_window_start: std::time::Instant,
    current_fps: f64,

    game_flow: GameFlowWatcher,
    
    is_host: bool,

    racer_mirror: Option<writer::RacerMirrorSocket>,
    
    udp_server_addr: String,
    
    my_server_slot: Option<wram::ServerSlot>,
    
    slot_map: wram::SlotMap,
    
    known_start_pos: std::collections::HashMap<wram::ServerSlot, u8>,

    pending_start_pos: Option<u8>,

    pending_track_index: Option<u8>,
    
    track_index_injected: bool,

    auto_finisher: writer::AutoFinisher,

    race_start_time: Option<std::time::Instant>,

    awaiting_fold_in: bool,
    
    room_racing: bool,

    this_race_human_slots: Vec<u8>,
}

impl AppState {
    
    fn new(emu: Emulator, lobby: LobbyApp, udp_server_addr: &str) -> Self {
        
        let sample_rate = emu.get_audio_sample_rate();
        let audio = match AudioOutput::new(sample_rate) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("audio disabled: {e}");
                None
            }
        };

        Self {
            emu,
            framebuffer_argb: vec![0; WIDTH * HEIGHT],
            framebuffer_rgba8: vec![0; WIDTH * HEIGHT * 4],
            frame_dims: (WIDTH, HEIGHT),
            texture: None,
            lobby,
            audio,
            last_tick: std::time::Instant::now(),
            accumulator: std::time::Duration::ZERO,

            settings: Settings::load(),
            settings_window: SettingsWindow::new(),
            gilrs: match Gilrs::new() {
                Ok(g) => Some(g),
                Err(e) => {
                    eprintln!("gamepad input disabled: {e}");
                    None
                }
            },

            fps_frame_count: 0,
            fps_window_start: std::time::Instant::now(),
            current_fps: TARGET_FPS,

            game_flow: GameFlowWatcher::new(),
            is_host: false,

            racer_mirror: None,
            udp_server_addr: udp_server_addr.to_string(),
            my_server_slot: None,
            slot_map: wram::SlotMap::new(),
            known_start_pos: std::collections::HashMap::new(),
            pending_start_pos: None,

            pending_track_index: None,
            track_index_injected: false,

            auto_finisher: writer::AutoFinisher::new(),
            race_start_time: None,

            awaiting_fold_in: false,
            room_racing: false,
            this_race_human_slots: vec![wram::LOCAL_KART_SLOT],
        }
    }

    fn human_kart_slots(&self) -> Vec<u8> {
        let mut slots: Vec<u8> = vec![wram::LOCAL_KART_SLOT];
        slots.extend(self.slot_map.iter().map(|(_, kart_slot)| kart_slot));
        slots
    }

    fn rebuild_slot_map(&mut self, active: &[net::RoomMember], pending: &[net::RoomMember]) {
        let Some(my_slot) = self.my_server_slot else { return };

        let mut present: std::collections::HashSet<wram::ServerSlot> = std::collections::HashSet::new();
        for member in active.iter().chain(pending.iter()) {
            if member.slot < 0 {
                continue;
            }
            let slot = member.slot as wram::ServerSlot;
            if slot == my_slot {
                continue;
            }
            present.insert(slot);
            self.slot_map.assign(slot);
            
            self.known_start_pos.insert(slot, member.start_pos as u8);
        }

        for slot in 0..wram::NUM_KART_SLOTS {
            if slot != my_slot && self.slot_map.get(slot).is_some() && !present.contains(&slot) {
                self.slot_map.free(slot);
                self.known_start_pos.remove(&slot);
            }
        }
    }
}

impl AppState {
    
    fn ensure_racer_mirror(&mut self, udp_token: &str) {
        if self.racer_mirror.is_some() {
            return;
        }

        let token_bytes = match hex::decode(udp_token) {
            Ok(b) if b.len() == 8 => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&b);
                arr
            }
            Ok(b) => {
                eprintln!(
                    "ensure_racer_mirror: udp_token {udp_token:?} decoded to {} bytes, expected 8 -- refusing to open UDP mirror",
                    b.len()
                );
                return;
            }
            Err(e) => {
                eprintln!("ensure_racer_mirror: failed to hex-decode udp_token {udp_token:?}: {e}");
                return;
            }
        };

        match writer::RacerMirrorSocket::new(UDP_BIND_ADDR, &self.udp_server_addr, token_bytes) {
            Ok(mirror) => {
                eprintln!(
                    "ensure_racer_mirror: UDP racer mirror bound on {UDP_BIND_ADDR}, sending to {}",
                    self.udp_server_addr
                );
                self.racer_mirror = Some(mirror);
            }
            Err(e) => {
                eprintln!(
                    "ensure_racer_mirror: failed to bind RacerMirrorSocket (bind {UDP_BIND_ADDR}, server {}): {e}",
                    self.udp_server_addr
                );
            }
        }
    }

    fn handle_server_events_for_wram(&mut self, events: &[ServerEvent]) {
        
        for event in events {
            match event {
                ServerEvent::RoomCreated { start_pos, udp_token, .. } => {
                    
                    self.awaiting_fold_in = *start_pos <= 0;
                    if *start_pos > 0 {
                        self.pending_start_pos = Some(*start_pos as u8);
                        self.my_server_slot = Some((*start_pos - 1) as wram::ServerSlot);
                    }
                    self.ensure_racer_mirror(udp_token);
                }
                ServerEvent::Joined { start_pos, udp_token, .. } => {
                    
                    self.awaiting_fold_in = *start_pos <= 0;
                    
                    if *start_pos > 0 {
                        self.pending_start_pos = Some(*start_pos as u8);
                        self.my_server_slot = Some((*start_pos - 1) as wram::ServerSlot);
                    }
                    self.ensure_racer_mirror(udp_token);
                }
                ServerEvent::FoldedIn { start_pos, next_track } => {
                    
                    self.awaiting_fold_in = false;
                    if *start_pos > 0 {
                        self.pending_start_pos = Some(*start_pos as u8);
                        self.my_server_slot = Some((*start_pos - 1) as wram::ServerSlot);
                    }
                    
                    if !self.track_index_injected {
                        self.pending_track_index = Some(*next_track as u8);
                    }
                }
                ServerEvent::RoomState { active, pending, status, next_track, .. } => {
                    self.room_racing = status == "racing";
                    
                    if self.room_racing && !self.track_index_injected && self.pending_track_index.is_none() {
                        self.pending_track_index = Some(*next_track as u8);
                    }
                    self.rebuild_slot_map(active, pending);
                }
                ServerEvent::RaceStart { slot, start_pos, .. } => {
                    
                    if self.my_server_slot.is_none() {
                        self.my_server_slot = Some(*slot as wram::ServerSlot);
                    }
                    
                    if let Some(sp) = start_pos {
                        self.pending_start_pos = Some(*sp as u8);
                    }
                }
                
                ServerEvent::Left
                | ServerEvent::Kicked
                | ServerEvent::RoomClosed { .. }
                | ServerEvent::Disconnected => {
                    self.is_host = false;
                    self.awaiting_fold_in = false;
                    self.room_racing = false;
                    self.my_server_slot = None;
                    self.pending_start_pos = None;
                    self.pending_track_index = None;
                    self.track_index_injected = false;
                }
                _ => {}
            }
        }

        let ram = self.emu.system_ram_mut();
        for event in events {
            match event {
                ServerEvent::RoomState { status, is_host, all_active_ready, .. } => {
                    
                    if let Some(host) = is_host {
                        self.is_host = *host;
                    }
                    
                    let can_enter = match is_host {
                        Some(true) => *all_active_ready,
                        
                        Some(false) | None => status == "racing" && !self.awaiting_fold_in,
                    };
                    writer::write_mailbox_bit(ram, wram::CAN_ENTER_RACE_BIT, can_enter);
                    
                    self.lobby.set_mushroom_prompt(can_enter);

                }
                
                ServerEvent::Left
                | ServerEvent::Kicked
                | ServerEvent::RoomClosed { .. }
                | ServerEvent::Disconnected => {
                    writer::write_mailbox_bit(ram, wram::CAN_ENTER_RACE_BIT, false);
                    self.lobby.set_mushroom_prompt(false);
                }
                ServerEvent::RaceStart { rng_seed, .. } => {
                    
                    writer::write_rng_seed(ram, *rng_seed as u16);
                    
                    let before = if wram::MAILBOX_ADDR < ram.len() { Some(ram[wram::MAILBOX_ADDR]) } else { None };
                    writer::write_mailbox_bit(ram, wram::ALL_READY_BIT, true);
                    let after = if wram::MAILBOX_ADDR < ram.len() { Some(ram[wram::MAILBOX_ADDR]) } else { None };
                    eprintln!(
                        "RaceStart event: rng_seed={rng_seed:#06x} mailbox before={before:?} after={after:?} (ALL_READY_BIT={:#04x})",
                        wram::ALL_READY_BIT
                    );

                    eprintln!("RaceStart event -- starting race timeout clock ({RACE_TIMEOUT:?})");
                    self.race_start_time = Some(std::time::Instant::now());
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        if let Some(gilrs) = &mut self.gilrs {
            while gilrs.next_event().is_some() {}
        }

        let p1 = self.settings.read_buttons(ctx, self.gilrs.as_ref());
        let p2 = Buttons::new();

        let now = std::time::Instant::now();
        self.accumulator += now.duration_since(self.last_tick);
        self.last_tick = now;

        let frame_duration = std::time::Duration::from_secs_f64(1.0 / TARGET_FPS);
        
        const MAX_CATCHUP_FRAMES: u32 = 4;
        let mut stepped = 0;
        while self.accumulator >= frame_duration && stepped < MAX_CATCHUP_FRAMES {
            self.emu.run([p1, p2]);

            if let Some(audio) = &self.audio {
                
                let _ = self.emu.peek_audio_sample(|samples| audio.push_samples(samples));
            }

            self.accumulator -= frame_duration;
            stepped += 1;
        }
        if stepped == MAX_CATCHUP_FRAMES {
            
            self.accumulator = std::time::Duration::ZERO;
        }

        self.fps_frame_count += stepped;
        let fps_elapsed = now.duration_since(self.fps_window_start);
        if fps_elapsed >= std::time::Duration::from_secs(1) {
            self.current_fps = self.fps_frame_count as f64 / fps_elapsed.as_secs_f64();
            self.fps_frame_count = 0;
            self.fps_window_start = now;
        }

        if self.lobby.take_settings_clicked() {
            self.settings_window.toggle();
        }
        let prev_volume = self.settings.volume;
        
        let settings_before = self.settings.clone();
        self.settings_window.draw(ctx, &mut self.settings, self.gilrs.as_ref());
        if self.settings.volume != prev_volume {
            if let Some(audio) = &self.audio {
                audio.set_volume(self.settings.volume);
            }
        }
        if self.settings != settings_before {
            self.settings.save();
        }

        let (real_w, real_h) = self.emu.framebuffer_size();
        if (real_w, real_h) != self.frame_dims && real_w > 0 && real_h > 0 {
            self.framebuffer_argb = vec![0; real_w * real_h];
            self.framebuffer_rgba8 = vec![0; real_w * real_h * 4];
            self.frame_dims = (real_w, real_h);
            
            self.texture = None;
        }
        let (fb_w, fb_h) = self.frame_dims;

        if let Err(e) = self.emu.copy_framebuffer_argb32(&mut self.framebuffer_argb) {
            eprintln!("framebuffer copy failed: {e:?}");
        } else {
            for (i, &argb) in self.framebuffer_argb.iter().enumerate() {
                let a = (argb >> 24) as u8;
                let r = (argb >> 16) as u8;
                let g = (argb >> 8) as u8;
                let b = argb as u8;
                let o = i * 4;
                self.framebuffer_rgba8[o] = r;
                self.framebuffer_rgba8[o + 1] = g;
                self.framebuffer_rgba8[o + 2] = b;
                self.framebuffer_rgba8[o + 3] = if a == 0 { 255 } else { a };
                
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([fb_w, fb_h], &self.framebuffer_rgba8);
            match &mut self.texture {
                Some(tex) => tex.set(image, egui::TextureOptions::NEAREST),
                None => {
                    self.texture =
                        Some(ctx.load_texture("emulator_framebuffer", image, egui::TextureOptions::NEAREST));
                }
            }
        }

        let total_width = ctx.screen_rect().width();
        let side_panel_width = (total_width * 0.25).max(180.0);

        let mut lobby_events: Vec<ServerEvent> = Vec::new();
        egui::SidePanel::right("lobby_panel")
            .resizable(false)
            .exact_width(side_panel_width)
            .show(ctx, |ui| {
                lobby_events = self.lobby.ui(ui);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    let color = if self.current_fps < 59.0 {
                        egui::Color32::RED
                    } else {
                        egui::Color32::from_gray(140)
                    };
                    ui.colored_label(color, format!("{:.0} FPS", self.current_fps));
                });
            });
        self.handle_server_events_for_wram(&lobby_events);

        if self.is_host && !self.track_index_injected {
            self.pending_track_index = Some(self.lobby.start_track());
        }

        if let Some(track_index) = self.pending_track_index {
            let ram = self.emu.system_ram_mut();
            writer::inject_track_index(ram, track_index);
        }

        let joined_this_frame = lobby_events.iter().any(|e| {
            matches!(e, ServerEvent::Joined { .. } | ServerEvent::RoomCreated { .. })
        });

        let wram_requests = {
            let ram = self.emu.system_ram_ref();
            self.game_flow.poll(
                ram,
                self.is_host,
                joined_this_frame,
                self.room_racing,
                self.lobby.start_track(),
            )
        };
        for request in wram_requests {
            self.lobby.send(request);
        }

        if self.game_flow.race_started_fired {
            eprintln!("race_started_fired -- clearing ALL_READY_BIT (lap byte hit 0x80)");
            let ram = self.emu.system_ram_mut();
            writer::write_mailbox_bit(ram, wram::ALL_READY_BIT, false);
        }

        if self.game_flow.entered_race_edge {
            eprintln!(
                "entered_race_edge fired, pending_start_pos = {:?}",
                self.pending_start_pos
            );
            
            self.pending_track_index = None;
            self.track_index_injected = true;

            {
                let ram = self.emu.system_ram_mut();
                writer::write_mailbox_bit(ram, wram::CAN_ENTER_RACE_BIT, false);
            }
            self.lobby.set_mushroom_prompt(false);

            self.auto_finisher.reset();
            
            self.race_start_time = None;
            
            if let Some(mirror) = &mut self.racer_mirror {
                mirror.reset_for_new_race();
            }

            self.this_race_human_slots = self.human_kart_slots();
            let occupied_kart_slots = self.this_race_human_slots.clone();
            {
                let ram = self.emu.system_ram_mut();
                writer::remove_unoccupied_cpus(ram, &occupied_kart_slots);
            }

            if let Some(my_start_pos) = self.pending_start_pos.take() {
                let others: Vec<(u8, u8)> = self
                    .slot_map
                    .iter()
                    .filter_map(|(server_slot, kart_slot)| {
                        self.known_start_pos
                            .get(&server_slot)
                            .map(|&start_pos| (kart_slot, start_pos))
                    })
                    .collect();
                let ram = self.emu.system_ram_mut();
                writer::run_startup_placement(ram, my_start_pos, &others);
            }
        }

        let in_race = {
            let ram = self.emu.system_ram_ref();
            ram.len() > wram::GAMEMODE_ADDR && ram[wram::GAMEMODE_ADDR] == wram::GAMEMODE_IN_RACE
        };
        if in_race {
            if let (Some(mirror), Some(my_slot)) = (&mut self.racer_mirror, self.my_server_slot) {
                mirror.poll_and_apply(self.emu.system_ram_mut(), &self.slot_map, my_slot);
                
                if self.game_flow.should_broadcast {
                    mirror.send_local(self.emu.system_ram_ref(), my_slot);
                }
                
                let known_remotes: Vec<(wram::ServerSlot, u8)> = self
                    .slot_map
                    .iter()
                    .filter(|&(_, kart_slot)| self.this_race_human_slots.contains(&kart_slot))
                    .collect();
                mirror.hide_pending_remote_slots(self.emu.system_ram_mut(), &known_remotes);
            }

            let human_kart_slots = self.this_race_human_slots.clone();
            writer::apply_non_collision_to_cpus(self.emu.system_ram_mut(), &human_kart_slots);

            let race_timed_out = self
                .race_start_time
                .is_some_and(|started| started.elapsed() >= RACE_TIMEOUT);
            if race_timed_out {
                self.auto_finisher.force_finish_all(self.emu.system_ram_mut());
            } else {
                self.auto_finisher.poll(self.emu.system_ram_mut(), &human_kart_slots);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tex) = &self.texture {
                
                let avail = ui.available_size();
                let native_aspect = fb_w as f32 / fb_h as f32;
                let mut size = egui::vec2(fb_w as f32 * 2.0, fb_h as f32 * 2.0);
                if avail.x < size.x || avail.y < size.y {
                    
                    let scale = (avail.x / size.x).min(avail.y / size.y);
                    size *= scale;
                }
                let _ = native_aspect;
                ui.add(egui::Image::new(tex).fit_to_exact_size(size));
            }
        });

        ctx.request_repaint_after(frame_duration);
    }
}

fn main() {
    let vanilla_rom = locate_vanilla_rom();
    let patched_rom = build_patched_rom(&vanilla_rom);

    let core_filename = if cfg!(target_os = "windows") {
        "snes9x_libretro.dll"
    } else if cfg!(target_os = "macos") {
        "snes9x_libretro.dylib"
    } else {
        "snes9x_libretro.so"
    };
    let core_path = exe_dir().join("cores/snes9x-libretro").join(core_filename);                               
    if !core_path.exists() {
        eprintln!("Core not found at {core_path:?}. Check the path in main.rs.");
        std::process::exit(1);
    }

    let emu = Emulator::create(&core_path, &patched_rom);

    let server_addr = if std::env::args().any(|a| a == "--local") {
        LOCAL_SERVER_ADDR.to_string()
    } else {
        std::env::var("SMK_SERVER_ADDR").unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
    };
    let udp_broadcast_addr =
        std::env::var("SMK_UDP_BROADCAST_ADDR").unwrap_or_else(|_| DEFAULT_UDP_BROADCAST_ADDR.to_string());
    eprintln!("net: lobby server address: {server_addr}");
    eprintln!("net: UDP racer-mirror broadcast address: {udp_broadcast_addr}");

    let lobby = LobbyApp::new(server_addr);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([WIDTH as f32 * 2.0 + 280.0, HEIGHT as f32 * 2.0 + 40.0]);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions { viewport, ..Default::default() };

    eframe::run_native(
        "SMK8P",
        native_options,
        Box::new(move |_cc| Ok(Box::new(AppState::new(emu, lobby, &udp_broadcast_addr)))),
    )
    .expect("eframe failed");
}