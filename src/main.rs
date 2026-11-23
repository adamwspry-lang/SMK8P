mod audio;
mod lobby;
mod net;
mod rng;
mod settings;
mod wram;
mod writer;

use audio::AudioOutput;
use eframe::egui;
use gilrs::Gilrs;
use lobby::LobbyApp;
use net::ServerEvent;
use retro_rs::{Buttons, Emulator};
use rng::{fish, hazard_rng, mole, thwomp};
use settings::{Settings, SettingsWindow};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const WIDTH: usize = 256;
const HEIGHT: usize = 224;
const TARGET_FPS: f64 = 60.098_812;
const MAX_CATCHUP_FRAMES: u32 = 4;
const RACE_TIMEOUT: Duration = Duration::from_secs(220);
const THWOMP_ACTIVATION_DELAY_FRAMES: u32 = 37;
const LIGHTNING_STRIKE_BIT: u8 = 0x08;

const KNOWN_GOOD_SHA1: &str = "47e103d8398cf5b7cbb42b95df3a3c270691163b";
const ROM_LOCATION_FILE: &str = "roms/rom_location.txt";
const PATCHED_ROM_PATH: &str = "roms/smk_patched.sfc";

const DEFAULT_SERVER_ADDR: &str = "2.29.45.28:9000";
const LOCAL_SERVER_ADDR: &str = "127.0.0.1:9000";
const UDP_BIND_ADDR: &str = "0.0.0.0:0";
const DEFAULT_UDP_BROADCAST_ADDR: &str = "2.29.45.28:9001";

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon_256.png");

include!(concat!(env!("OUT_DIR"), "/embedded_patches.rs"));

pub(crate) fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .expect("failed to get current exe path")
        .parent()
        .expect("exe has no parent dir")
        .to_path_buf()
}

fn locate_core_path(core_filename: &str) -> PathBuf {
    let release_path = exe_dir().join("cores/snes9x-libretro").join(core_filename);
    if release_path.exists() {
        return release_path;
    }
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("cores/snes9x-libretro")
        .join(core_filename);
    if dev_path.exists() {
        return dev_path;
    }
    release_path
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
                    if hash != KNOWN_GOOD_SHA1 {
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
        .pick_file()
        .unwrap_or_else(|| {
            eprintln!("No file selected. Exiting.");
            std::process::exit(1);
        });

    match sha1_hex(&picked) {
        Ok(hash) => {
            if hash != KNOWN_GOOD_SHA1 {
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

fn build_patched_rom(vanilla_path: &Path) -> PathBuf {
    let out_path = exe_dir().join(PATCHED_ROM_PATH);

    let mut rom = match fs::read(&out_path) {
        Ok(bytes) => bytes,
        Err(_) => fs::read(vanilla_path).expect("failed to read vanilla ROM"),
    };

    for (name, patch_bytes) in EMBEDDED_PATCHES {
        let records = match parse_ips(patch_bytes) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse embedded patch {name}: {e}. Skipping it.");
                continue;
            }
        };
        if !is_patch_applied(&rom, &records) {
            apply_records(&mut rom, &records);
        }
    }

    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&out_path, &rom).expect("failed to write patched ROM");
    out_path
}

struct GameFlowWatcher {
    prev_gamemode: u8,
    prev_current_lap: u8,
    prev_lakitu_script_ptr: u16,
    prev_pause_menu_open: bool,
    has_readied_this_select: bool,
    has_reported_lap2_this_race: bool,
    race_started_fired: bool,
    entered_race_edge: bool,
    should_broadcast: bool,
    left_room_fired: bool,
    at_title_screen: bool,
}

impl GameFlowWatcher {
    fn new() -> Self {
        Self {
            prev_gamemode: wram::GAMEMODE_NONE,
            prev_current_lap: 0,
            prev_lakitu_script_ptr: 0,
            prev_pause_menu_open: false,
            has_readied_this_select: false,
            has_reported_lap2_this_race: false,
            race_started_fired: false,
            entered_race_edge: false,
            should_broadcast: true,
            left_room_fired: false,
            at_title_screen: false,
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
        self.left_room_fired = false;

        let local_lap_addr = wram::racer_offset(wram::LOCAL_KART_SLOT) + wram::CURRENT_LAP_OFFSET;

        if ram.len() <= wram::GAMEMODE_ADDR
            || ram.len() <= local_lap_addr
            || ram.len() <= wram::LAKITU_SCRIPT_PTR_ADDR + 1
            || ram.len() <= wram::PAUSE_MENU_ADDR
        {
            eprintln!(
                "GameFlowWatcher::poll: WRAM slice too small (len {:#x}) to read gamemode ({:#x}), local current-lap ({:#x}), Lakitu script pointer ({:#x}), or pause-menu flag ({:#x}) -- core may not have WRAM mapped yet",
                ram.len(),
                wram::GAMEMODE_ADDR,
                local_lap_addr,
                wram::LAKITU_SCRIPT_PTR_ADDR,
                wram::PAUSE_MENU_ADDR
            );
            return out;
        }

        let gm = ram[wram::GAMEMODE_ADDR];
        self.at_title_screen = gm == wram::GAMEMODE_TITLE;

        if joined_this_frame {
            self.has_readied_this_select = false;
        }

        if gm == wram::GAMEMODE_TRACK_SELECT && !room_racing && !self.has_readied_this_select {
            out.push(net::Request::Ready);
            self.has_readied_this_select = true;
        } else if gm != wram::GAMEMODE_TRACK_SELECT {
            self.has_readied_this_select = false;
        }

        if gm != wram::GAMEMODE_NONE && gm != self.prev_gamemode {
            if gm == wram::GAMEMODE_IN_RACE {
                self.entered_race_edge = true;
                self.prev_current_lap = 0xFF;
                self.prev_lakitu_script_ptr = 0;
                self.has_reported_lap2_this_race = false;
            }
            self.prev_gamemode = gm;
        }

        let pause_menu_open = ram[wram::PAUSE_MENU_ADDR] & wram::PAUSE_MENU_OPEN_BIT != 0;
        let pause_menu_opened_edge = pause_menu_open && !self.prev_pause_menu_open;
        self.prev_pause_menu_open = pause_menu_open;

        if gm == wram::GAMEMODE_IN_RACE && pause_menu_opened_edge {
            out.push(net::Request::LeaveRoom);
            self.left_room_fired = true;
        }

        let lakitu_ptr =
            u16::from_le_bytes([ram[wram::LAKITU_SCRIPT_PTR_ADDR], ram[wram::LAKITU_SCRIPT_PTR_ADDR + 1]]);
        if gm == wram::GAMEMODE_IN_RACE
            && lakitu_ptr == wram::LAKITU_PARKED_VALUE
            && self.prev_lakitu_script_ptr != wram::LAKITU_PARKED_VALUE
        {
            out.push(net::Request::Ready);
        }
        self.prev_lakitu_script_ptr = lakitu_ptr;

        let current_lap = ram[local_lap_addr];

        self.race_started_fired =
            current_lap == wram::RACE_STARTED_LAP_VALUE && self.prev_current_lap != wram::RACE_STARTED_LAP_VALUE;

        if current_lap == wram::FINISHED_LAP_VALUE && self.prev_current_lap != wram::FINISHED_LAP_VALUE {
            out.push(net::Request::Finished);
        }

        if is_host
            && gm == wram::GAMEMODE_IN_RACE
            && current_lap == wram::LAP_NOT_STARTED_VALUE
            && self.prev_current_lap != wram::LAP_NOT_STARTED_VALUE
        {
            out.push(net::Request::StartRoom { start_track: start_track as i32 });
        }

        if current_lap == wram::FINISHED_LAP_VALUE && self.prev_current_lap == wram::FINISHED_LAP_VALUE {
            self.should_broadcast = false;
        } else if current_lap == wram::LAP_NOT_STARTED_VALUE {
            self.should_broadcast = true;
        }

        let entered_lap2_edge =
            current_lap == wram::CURRENT_LAP_LAP2_VALUE && self.prev_current_lap != wram::CURRENT_LAP_LAP2_VALUE;
        if entered_lap2_edge && !self.has_reported_lap2_this_race {
            out.push(net::Request::EnteredLap2);
            self.has_reported_lap2_this_race = true;
        }

        self.prev_current_lap = current_lap;

        out
    }
}

enum HazardSim {
    None,
    Fish {
        track: fish::Track,
        rng: hazard_rng::HazardRng,
        fish: [fish::Fish; 4],
    },
    Thwomp {
        rng: hazard_rng::HazardRng,
        thwomps: [thwomp::Thwomp; 4],
        frames_until_activate: Option<u32>,
    },
    Moles {
        rng: hazard_rng::HazardRng,
        moles: [mole::Mole; 4],
    },
}

impl HazardSim {
    fn for_track(track_index: u8, seed: u16) -> HazardSim {
        if track_index == wram::TRACK_ID_FISH_KB1 || track_index == wram::TRACK_ID_FISH_KB2 {
            let track = if track_index == wram::TRACK_ID_FISH_KB1 { fish::Track::Kb1 } else { fish::Track::Kb2 };
            let targets = track.targets();
            let fish = [
                fish::Fish::spawn(targets[0].0, targets[0].1),
                fish::Fish::spawn(targets[1].0, targets[1].1),
                fish::Fish::spawn(targets[2].0, targets[2].1),
                fish::Fish::spawn(targets[3].0, targets[3].1),
            ];
            HazardSim::Fish { track, rng: hazard_rng::HazardRng::from_seed(seed), fish }
        } else if wram::TRACK_IDS_THWOMP.contains(&track_index) {
            HazardSim::Thwomp {
                rng: hazard_rng::HazardRng::from_seed(seed),
                thwomps: [thwomp::Thwomp::spawn(); 4],
                frames_until_activate: None,
            }
        } else if wram::TRACK_IDS_MOLES.contains(&track_index) {
            HazardSim::Moles {
                rng: hazard_rng::HazardRng::from_seed(seed),
                moles: [mole::Mole::spawn(); 4],
            }
        } else {
            HazardSim::None
        }
    }

    fn step(&mut self, lap2_signal_this_frame: bool) {
        match self {
            HazardSim::None => {}
            HazardSim::Fish { track, rng, fish } => {
                for &slot_index in track.dispatch_order().iter() {
                    fish[slot_index].step(rng);
                }
            }
            HazardSim::Thwomp { rng, thwomps, frames_until_activate } => {
                if lap2_signal_this_frame && frames_until_activate.is_none() {
                    *frames_until_activate = Some(THWOMP_ACTIVATION_DELAY_FRAMES);
                }
                let activate_now = match frames_until_activate {
                    Some(0) => true,
                    Some(n) => {
                        *n -= 1;
                        false
                    }
                    None => false,
                };
                for &slot_index in thwomp::BC2_DISPATCH_ORDER.iter() {
                    thwomps[slot_index].step(rng, activate_now);
                }
                rng.next();
            }
            HazardSim::Moles { rng, moles } => {
                for &slot_index in mole::MOLE_DISPATCH_ORDER.iter() {
                    moles[slot_index].step(rng);
                }
                rng.next();
            }
        }
    }

    fn write_into(&self, ram: &mut [u8]) {
        match self {
            HazardSim::None => {}
            HazardSim::Fish { fish, .. } => {
                for (slot_index, f) in fish.iter().enumerate() {
                    writer::write_fish_state(
                        ram, slot_index, f.pos0, f.vel0, f.pos1, f.frac1, f.vel1, f.tgt1, f.pos2, f.frac2, f.vel2,
                        f.tgt2,
                    );
                }
            }
            HazardSim::Thwomp { thwomps, .. } => {
                for (slot_index, t) in thwomps.iter().enumerate() {
                    writer::write_thwomp_state(
                        ram,
                        slot_index,
                        t.pos,
                        t.frac,
                        t.vel,
                        t.phase.state_code(),
                        t.remaining,
                    );
                }
            }
            HazardSim::Moles { moles, .. } => {
                for (slot_index, m) in moles.iter().enumerate() {
                    writer::write_mole_state(ram, slot_index, m.pos, m.vel, m.phase.state_code(), m.cd);
                }
            }
        }
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

    last_tick: Instant,
    accumulator: Duration,

    settings: Settings,
    settings_window: SettingsWindow,
    gilrs: Option<Gilrs>,

    fps_frame_count: u32,
    fps_window_start: Instant,
    current_fps: f64,

    game_flow: GameFlowWatcher,
    is_host: bool,

    racer_mirror: Option<writer::RacerMirrorSocket>,
    udp_server_addr: String,
    my_server_slot: Option<wram::ServerSlot>,
    slot_map: wram::SlotMap,
    known_start_pos: HashMap<wram::ServerSlot, u8>,

    pending_start_pos: Option<u8>,
    pending_track_index: Option<u8>,
    track_index_injected: bool,
    pending_rng_seed: Option<u16>,
    prev_loading_screen_bool: u16,

    auto_finisher: writer::AutoFinisher,
    race_start_time: Option<Instant>,

    awaiting_fold_in: bool,
    room_racing: bool,
    this_race_human_slots: Vec<u8>,
    awaiting_title_before_rejoin: bool,
    removed_cpu_originals: HashMap<u8, (u16, u16)>,

    throw_watcher: writer::ThrowWatcher,
    pending_item_spawns: VecDeque<(u8, u8, bool)>,
    lightning_watcher: writer::LightningWatcher,
    pending_lightning_uses: VecDeque<u8>,
    remote_lightning_in_flight: bool,
    consume_watcher: writer::ConsumeWatcher,
    despawn_watcher: writer::DespawnWatcher,
    collectible_watcher: writer::CollectibleWatcher,
    pristine_tile_map: writer::PristineTileMap,
    pending_collectibles: VecDeque<(u16, writer::CollectibleKind, u8)>,

    hazard_sim: HazardSim,
    hazard_active: bool,
    race_track_index: Option<u8>,
    race_rng_seed: Option<u16>,
    lap2_signal_pending: bool,

    sync_item_boxes: bool,
    sync_coins: bool,
    sync_rng: bool,
}

impl AppState {
    fn new(emu: Emulator, lobby: LobbyApp, udp_server_addr: &str) -> Self {
        let sample_rate = emu.get_audio_sample_rate();
        let settings = Settings::load();
        let audio = match AudioOutput::new(sample_rate, settings.volume) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("audio disabled: {e}");
                None
            }
        };
        let gilrs = match Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("gamepad input disabled: {e}");
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

            last_tick: Instant::now(),
            accumulator: Duration::ZERO,

            settings,
            settings_window: SettingsWindow::new(),
            gilrs,

            fps_frame_count: 0,
            fps_window_start: Instant::now(),
            current_fps: TARGET_FPS,

            game_flow: GameFlowWatcher::new(),
            is_host: false,

            racer_mirror: None,
            udp_server_addr: udp_server_addr.to_string(),
            my_server_slot: None,
            slot_map: wram::SlotMap::new(),
            known_start_pos: HashMap::new(),

            pending_start_pos: None,
            pending_track_index: None,
            track_index_injected: false,
            pending_rng_seed: None,
            prev_loading_screen_bool: 0xFFFF,

            auto_finisher: writer::AutoFinisher::new(),
            race_start_time: None,

            awaiting_fold_in: false,
            room_racing: false,
            this_race_human_slots: vec![wram::LOCAL_KART_SLOT],
            awaiting_title_before_rejoin: false,
            removed_cpu_originals: HashMap::new(),

            throw_watcher: writer::ThrowWatcher::new(),
            pending_item_spawns: VecDeque::new(),
            lightning_watcher: writer::LightningWatcher::new(),
            pending_lightning_uses: VecDeque::new(),
            remote_lightning_in_flight: false,
            consume_watcher: writer::ConsumeWatcher::new(),
            despawn_watcher: writer::DespawnWatcher::new(),
            collectible_watcher: writer::CollectibleWatcher::new(),
            pristine_tile_map: writer::PristineTileMap::new(),
            pending_collectibles: VecDeque::new(),

            hazard_sim: HazardSim::None,
            hazard_active: false,
            race_track_index: None,
            race_rng_seed: None,
            lap2_signal_pending: false,

            sync_item_boxes: true,
            sync_coins: true,
            sync_rng: true,
        }
    }

    fn human_kart_slots(&self) -> Vec<u8> {
        let mut slots: Vec<u8> = vec![wram::LOCAL_KART_SLOT];
        slots.extend(self.slot_map.iter().map(|(_, kart_slot)| kart_slot));
        slots
    }

    fn rebuild_slot_map(
        &mut self,
        active: &[net::RoomMember],
        pending: &[net::RoomMember],
    ) -> Vec<(wram::ServerSlot, u8)> {
        let Some(my_slot) = self.my_server_slot else { return Vec::new() };

        let mut present: HashSet<wram::ServerSlot> = HashSet::new();
        let mut newly_assigned = Vec::new();
        for member in active.iter().chain(pending.iter()) {
            if member.slot < 0 {
                continue;
            }
            let slot = member.slot as wram::ServerSlot;
            if slot == my_slot {
                continue;
            }
            present.insert(slot);
            let already_known = self.slot_map.get(slot).is_some();
            if let Some(kart_slot) = self.slot_map.assign(slot) {
                if !already_known {
                    newly_assigned.push((slot, kart_slot));
                }
            }
            self.known_start_pos.insert(slot, member.start_pos as u8);
        }

        for slot in 0..wram::NUM_KART_SLOTS {
            if slot != my_slot && self.slot_map.get(slot).is_some() && !present.contains(&slot) {
                self.slot_map.free(slot);
                self.known_start_pos.remove(&slot);
            }
        }

        newly_assigned
    }

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
            Ok(mirror) => self.racer_mirror = Some(mirror),
            Err(e) => {
                eprintln!(
                    "ensure_racer_mirror: failed to bind RacerMirrorSocket (bind {UDP_BIND_ADDR}, server {}): {e}",
                    self.udp_server_addr
                );
            }
        }
    }

    fn reset_session_state(&mut self) {
        self.is_host = false;
        self.awaiting_fold_in = false;
        self.room_racing = false;
        self.my_server_slot = None;
        self.pending_start_pos = None;
        self.pending_track_index = None;
        self.track_index_injected = false;
        self.hazard_sim = HazardSim::None;
        self.race_track_index = None;
        self.race_rng_seed = None;
        self.lap2_signal_pending = false;
        self.hazard_active = false;
        self.sync_item_boxes = true;
        self.sync_coins = true;
        self.sync_rng = true;

        self.slot_map = wram::SlotMap::new();
        self.known_start_pos.clear();
        self.racer_mirror = None;
        self.this_race_human_slots = vec![wram::LOCAL_KART_SLOT];
        self.removed_cpu_originals.clear();
        self.reset_item_sync_state();

        self.awaiting_title_before_rejoin = true;
    }

    fn reset_item_sync_state(&mut self) {
        self.throw_watcher = writer::ThrowWatcher::new();
        self.pending_item_spawns.clear();
        self.lightning_watcher = writer::LightningWatcher::new();
        self.pending_lightning_uses.clear();
        self.remote_lightning_in_flight = false;
        self.consume_watcher = writer::ConsumeWatcher::new();
        self.despawn_watcher = writer::DespawnWatcher::new();
        self.collectible_watcher = writer::CollectibleWatcher::new();
        self.pristine_tile_map = writer::PristineTileMap::new();
        self.pending_collectibles.clear();
    }

    fn handle_server_events_for_wram(&mut self, events: &[ServerEvent]) {
        for event in events {
            match event {
                ServerEvent::RoomCreated { start_pos, udp_token, .. }
                | ServerEvent::Joined { start_pos, udp_token, .. } => {
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
                ServerEvent::RoomState {
                    active,
                    pending,
                    status,
                    next_track,
                    sync_item_boxes,
                    sync_coins,
                    sync_rng,
                    ..
                } => {
                    self.sync_item_boxes = *sync_item_boxes;
                    self.sync_coins = *sync_coins;
                    self.sync_rng = *sync_rng;
                    self.room_racing = status == "racing";
                    if self.room_racing && !self.track_index_injected && self.pending_track_index.is_none() {
                        self.pending_track_index = Some(*next_track as u8);
                    }
                    let newly_assigned = self.rebuild_slot_map(active, pending);

                    for (_, kart_slot) in &newly_assigned {
                        if let Some(original) = self.removed_cpu_originals.remove(kart_slot) {
                            let ram = self.emu.system_ram_mut();
                            writer::restore_cpu_slot(ram, *kart_slot, original);
                        }
                        if !self.this_race_human_slots.contains(kart_slot) {
                            self.this_race_human_slots.push(*kart_slot);
                        }
                    }

                    let still_human = self.human_kart_slots();
                    self.this_race_human_slots.retain(|slot| still_human.contains(slot));
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
                | ServerEvent::Disconnected => self.reset_session_state(),
                _ => {}
            }
        }

        let ram = self.emu.system_ram_mut();
        for event in events {
            match event {
                ServerEvent::RoomState { status, is_host, all_active_ready, next_rng_seed, .. } => {
                    if let Some(host) = is_host {
                        self.is_host = *host;
                    }
                    let can_enter = match is_host {
                        Some(true) => *all_active_ready,
                        Some(false) | None => status == "racing" && !self.awaiting_fold_in,
                    };
                    writer::write_mailbox_bit(ram, wram::CAN_ENTER_RACE_BIT, can_enter);
                    self.lobby.set_mushroom_prompt(can_enter);
                    self.pending_rng_seed = Some(*next_rng_seed as u16);
                }
                ServerEvent::Left
                | ServerEvent::Kicked
                | ServerEvent::RoomClosed { .. }
                | ServerEvent::Disconnected => {
                    writer::write_mailbox_bit(ram, wram::CAN_ENTER_RACE_BIT, false);
                    writer::write_mailbox_bit(ram, wram::ALL_READY_BIT, false);
                    self.lobby.set_mushroom_prompt(false);
                }
                ServerEvent::Lap2Entered => {
                    self.lap2_signal_pending = true;
                }
                ServerEvent::RaceStart { .. } => {
                    writer::write_mailbox_bit(ram, wram::ALL_READY_BIT, true);
                    self.race_start_time = Some(Instant::now());
                    self.hazard_active = true;
                }
                ServerEvent::ItemSpawn { thrower_slot, item_type, alt_direction, evicted_owner_slot } => {
                    let Ok(thrower_slot) = wram::ServerSlot::try_from(*thrower_slot) else {
                        eprintln!("ItemSpawn event: thrower_slot {thrower_slot} out of range -- dropping");
                        continue;
                    };
                    let Some(thrower_kart_slot) = self.slot_map.get(thrower_slot) else {
                        eprintln!(
                            "ItemSpawn event: no local kart-slot mapping yet for server_slot {thrower_slot} -- dropping (room_state may not have arrived yet)"
                        );
                        continue;
                    };

                    if let Some(evicted_owner_slot) = evicted_owner_slot {
                        match wram::ServerSlot::try_from(*evicted_owner_slot) {
                            Ok(evicted_server_slot) => {
                                let evicted_kart_slot = if self.my_server_slot == Some(evicted_server_slot) {
                                    Some(wram::LOCAL_KART_SLOT)
                                } else {
                                    self.slot_map.get(evicted_server_slot)
                                };
                                match evicted_kart_slot {
                                    Some(evicted_kart_slot) => {
                                        if !writer::item_owned_by_is_in_hit_animation(ram, evicted_kart_slot) {
                                            match writer::force_free_item_slot_owned_by(ram, evicted_kart_slot) {
                                                Some(freed_slot_index) => {
                                                    self.consume_watcher.suppress_slot(freed_slot_index)
                                                }
                                                None => eprintln!(
                                                    "ItemSpawn event: evicted_owner_slot server_slot {evicted_server_slot} (kart slot {evicted_kart_slot}) had no occupied item slot on our machine to free -- lists may have already diverged"
                                                ),
                                            }
                                        }
                                    }
                                    None => eprintln!(
                                        "ItemSpawn event: evicted_owner_slot server_slot {evicted_server_slot} has no local kart-slot mapping yet -- cannot free their item"
                                    ),
                                }
                            }
                            Err(_) => eprintln!(
                                "ItemSpawn event: evicted_owner_slot {evicted_owner_slot} out of range -- ignoring"
                            ),
                        }
                    }

                    self.pending_item_spawns.push_back((thrower_kart_slot, *item_type, *alt_direction));
                }
                ServerEvent::LightningUsed { thrower_slot } => {
                    let Ok(thrower_slot) = wram::ServerSlot::try_from(*thrower_slot) else {
                        eprintln!("LightningUsed event: thrower_slot {thrower_slot} out of range -- dropping");
                        continue;
                    };
                    let Some(thrower_kart_slot) = self.slot_map.get(thrower_slot) else {
                        eprintln!(
                            "LightningUsed event: no local kart-slot mapping yet for server_slot {thrower_slot} -- dropping (room_state may not have arrived yet)"
                        );
                        continue;
                    };
                    self.pending_lightning_uses.push_back(thrower_kart_slot);
                }
                ServerEvent::ItemUsed { owner_slot } => match wram::ServerSlot::try_from(*owner_slot) {
                    Ok(owner_server_slot) => {
                        let owner_kart_slot = if self.my_server_slot == Some(owner_server_slot) {
                            Some(wram::LOCAL_KART_SLOT)
                        } else {
                            self.slot_map.get(owner_server_slot)
                        };
                        match owner_kart_slot {
                            Some(owner_kart_slot) => {
                                if !writer::item_owned_by_is_in_hit_animation(ram, owner_kart_slot) {
                                    if let Some(freed_slot_index) =
                                        writer::force_free_item_slot_owned_by(ram, owner_kart_slot)
                                    {
                                        self.consume_watcher.suppress_slot(freed_slot_index);
                                    }
                                }
                            }
                            None => eprintln!(
                                "ItemUsed event: owner server_slot {owner_server_slot} has no local kart-slot mapping yet -- cannot free their item"
                            ),
                        }
                    }
                    Err(_) => eprintln!("ItemUsed event: owner_slot {owner_slot} out of range -- ignoring"),
                },
                ServerEvent::ItemDespawning { owner_slot } => match wram::ServerSlot::try_from(*owner_slot) {
                    Ok(owner_server_slot) => {
                        let owner_kart_slot = if self.my_server_slot == Some(owner_server_slot) {
                            Some(wram::LOCAL_KART_SLOT)
                        } else {
                            self.slot_map.get(owner_server_slot)
                        };
                        match owner_kart_slot {
                            Some(owner_kart_slot) => {
                                if let Some(slot_index) =
                                    writer::force_item_into_hit_animation_owned_by(ram, owner_kart_slot)
                                {
                                    self.despawn_watcher.suppress_slot(slot_index);
                                }
                            }
                            None => eprintln!(
                                "ItemDespawning event: owner server_slot {owner_server_slot} has no local kart-slot mapping yet -- cannot mirror despawn animation"
                            ),
                        }
                    }
                    Err(_) => eprintln!("ItemDespawning event: owner_slot {owner_slot} out of range -- ignoring"),
                },
                ServerEvent::CollectibleTaken { tile_index, tile_byte0, tile_byte1 } => {
                    let kind = if tile_byte1.is_some() {
                        writer::CollectibleKind::ItemBox
                    } else {
                        writer::CollectibleKind::Coin
                    };
                    let should_sync = match kind {
                        writer::CollectibleKind::ItemBox => self.sync_item_boxes,
                        writer::CollectibleKind::Coin => self.sync_coins,
                    };
                    if should_sync {
                        writer::apply_collectible_tile_bytes(
                            ram,
                            &self.pristine_tile_map,
                            *tile_index,
                            kind,
                            *tile_byte0,
                            *tile_byte1,
                        );
                        self.pending_collectibles.push_back((*tile_index, kind, *tile_byte0));
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_entered_race_edge(&mut self) {
        self.pending_track_index = None;
        self.track_index_injected = true;

        if let Some(seed) = self.pending_rng_seed.take() {
            eprintln!(
                "entered_race_edge: pending_rng_seed={seed:#06x} was NEVER injected (no loading-screen edge seen) -- RNG will NOT be in sync"
            );
        }

        self.race_track_index = self.emu.system_ram_ref().get(wram::RACE_NUMBER_ADDR).copied();

        self.hazard_sim = match (self.race_track_index, self.race_rng_seed.take()) {
            (Some(track_index), Some(seed)) => HazardSim::for_track(track_index, seed),
            _ => {
                if self.sync_rng {
                    eprintln!(
                        "entered_race_edge: missing race_track_index ({:?}) or race_rng_seed -- hazard objects unsynced this race",
                        self.race_track_index
                    );
                }
                HazardSim::None
            }
        };
        self.lap2_signal_pending = false;
        self.hazard_active = false;

        writer::write_mailbox_bit(self.emu.system_ram_mut(), wram::CAN_ENTER_RACE_BIT, false);
        self.lobby.set_mushroom_prompt(false);

        self.auto_finisher.reset();
        self.race_start_time = None;

        if let Some(mirror) = &mut self.racer_mirror {
            mirror.reset_for_new_race();
        }
        self.reset_item_sync_state();

        self.this_race_human_slots = self.human_kart_slots();
        let occupied_kart_slots = self.this_race_human_slots.clone();
        self.removed_cpu_originals =
            writer::remove_unoccupied_cpus(self.emu.system_ram_mut(), &occupied_kart_slots);

        if let Some(my_start_pos) = self.pending_start_pos.take() {
            let others: Vec<(u8, u8)> = self
                .slot_map
                .iter()
                .filter_map(|(server_slot, kart_slot)| {
                    self.known_start_pos.get(&server_slot).map(|&start_pos| (kart_slot, start_pos))
                })
                .collect();
            writer::run_startup_placement(self.emu.system_ram_mut(), my_start_pos, &others);
        }
    }

    fn run_in_race_sync(&mut self) {
        self.pristine_tile_map.capture_once(self.emu.system_ram_ref());

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

        let local_throw = self.throw_watcher.poll(self.emu.system_ram_ref());
        if let Some(writer::LocalThrow { item_type, alt_direction, evicted_remote_kart_slot }) = local_throw {
            let evicted_owner_slot = evicted_remote_kart_slot.and_then(|kart_slot| {
                let server_slot = self.slot_map.server_slot_for_kart(kart_slot);
                if server_slot.is_none() {
                    eprintln!(
                        "local throw evicted kart slot {kart_slot}, but it has no server_slot mapping -- not reporting the eviction"
                    );
                }
                server_slot.map(|s| s as i32)
            });
            self.lobby.send(net::Request::ThrowItem { item_type, alt_direction, evicted_owner_slot });
        }

        let (lightning_edge, lightning_flag_clear) = {
            let ram = self.emu.system_ram_ref();
            let strike_bit_clear =
                ram.get(wram::LIGHTNING_SFX_FLAG_ADDR).copied().unwrap_or(0) & LIGHTNING_STRIKE_BIT == 0;
            (
                self.lightning_watcher.poll(ram),
                !writer::lightning_request_pending(ram) && strike_bit_clear,
            )
        };
        if self.remote_lightning_in_flight {
            if lightning_flag_clear {
                self.remote_lightning_in_flight = false;
            }
        } else if lightning_edge {
            self.lobby.send(net::Request::LightningUsed);
        }

        let consumed_owners = self.consume_watcher.poll(self.emu.system_ram_ref());
        for owner_kart_slot in consumed_owners {
            match self.server_slot_for_owner(owner_kart_slot) {
                Some(owner_server_slot) => {
                    self.lobby.send(net::Request::ItemUsed { owner_slot: owner_server_slot as i32 });
                }
                None => eprintln!(
                    "item consumed for local kart slot {owner_kart_slot}, but it has no server_slot mapping -- not reporting"
                ),
            }
        }

        let despawning_owners = self.despawn_watcher.poll(self.emu.system_ram_ref());
        for owner_kart_slot in despawning_owners {
            match self.server_slot_for_owner(owner_kart_slot) {
                Some(owner_server_slot) => {
                    self.lobby.send(net::Request::ItemDespawning { owner_slot: owner_server_slot as i32 });
                }
                None => eprintln!(
                    "item entered despawn animation for local kart slot {owner_kart_slot}, but it has no server_slot mapping -- not reporting"
                ),
            }
        }

        let collectible_taken = self.collectible_watcher.poll(self.emu.system_ram_ref(), &self.pristine_tile_map);
        if let Some(writer::CollectibleTaken { tile_index, kind, byte0, byte1 }) = collectible_taken {
            let should_sync = match kind {
                writer::CollectibleKind::ItemBox => self.sync_item_boxes,
                writer::CollectibleKind::Coin => self.sync_coins,
            };
            if should_sync {
                self.lobby.send(net::Request::CollectibleTaken {
                    tile_index,
                    tile_byte0: byte0,
                    tile_byte1: byte1,
                });
            }
        }

        if let Some(&(kart_slot, item_type, alt_direction)) = self.pending_item_spawns.front() {
            let ram = self.emu.system_ram_mut();
            if writer::try_queue_item_throw(ram, kart_slot, item_type, alt_direction) {
                self.pending_item_spawns.pop_front();
            }
        }

        if let Some(&kart_slot) = self.pending_lightning_uses.front() {
            let ram = self.emu.system_ram_mut();
            if writer::try_queue_lightning_used(ram, kart_slot) {
                self.pending_lightning_uses.pop_front();
                self.remote_lightning_in_flight = true;
            }
        }

        if let Some(&(tile_index, kind, byte0)) = self.pending_collectibles.front() {
            let ram = self.emu.system_ram_mut();
            if writer::try_apply_collectible_vram_update(ram, &self.pristine_tile_map, tile_index, kind, byte0) {
                self.pending_collectibles.pop_front();
            }
        }

        let human_kart_slots = self.this_race_human_slots.clone();
        writer::apply_non_collision_to_cpus(self.emu.system_ram_mut(), &human_kart_slots);

        let race_timed_out = self.race_start_time.is_some_and(|started| started.elapsed() >= RACE_TIMEOUT);
        if race_timed_out {
            self.auto_finisher.force_finish_all(self.emu.system_ram_mut());
        } else {
            self.auto_finisher.poll(self.emu.system_ram_mut(), &human_kart_slots);
        }
    }

    fn server_slot_for_owner(&self, owner_kart_slot: u8) -> Option<wram::ServerSlot> {
        if owner_kart_slot == wram::LOCAL_KART_SLOT {
            self.my_server_slot
        } else {
            self.slot_map.server_slot_for_kart(owner_kart_slot)
        }
    }

    fn run_emulated_frames(&mut self, p1: Buttons, p2: Buttons) {
        let now = Instant::now();
        self.accumulator += now.duration_since(self.last_tick);
        self.last_tick = now;

        let frame_duration = Duration::from_secs_f64(1.0 / TARGET_FPS);

        let mut stepped = 0;
        while self.accumulator >= frame_duration && stepped < MAX_CATCHUP_FRAMES {
            self.emu.run([p1, p2]);

            if let Some(audio) = &self.audio {
                let _ = self.emu.peek_audio_sample(|samples| audio.push_samples(samples));
            }

            let ram = self.emu.system_ram_mut();
            if ram.len() > wram::LOADING_SCREEN_BOOL_ADDR + 1 {
                let loading_bool = u16::from_le_bytes([
                    ram[wram::LOADING_SCREEN_BOOL_ADDR],
                    ram[wram::LOADING_SCREEN_BOOL_ADDR + 1],
                ]);
                let entered_loading_edge = loading_bool == wram::LOADING_SCREEN_BOOL_LOADING
                    && self.prev_loading_screen_bool != wram::LOADING_SCREEN_BOOL_LOADING;
                self.prev_loading_screen_bool = loading_bool;

                if entered_loading_edge {
                    let seed = self.pending_rng_seed.take();
                    if self.sync_rng {
                        match seed {
                            Some(seed) => {
                                writer::write_rng_seed(ram, seed);
                                self.race_rng_seed = Some(seed);
                            }
                            None => eprintln!(
                                "Loading-screen edge ($7E:0120 -> 0x0000) fired, but no pending_rng_seed was set -- RNG will not be in sync this race"
                            ),
                        }
                    }
                }
            }

            let in_race_now = ram.get(wram::GAMEMODE_ADDR).copied() == Some(wram::GAMEMODE_IN_RACE);
            if in_race_now && self.hazard_active && self.sync_rng {
                let lap2_this_frame = std::mem::take(&mut self.lap2_signal_pending);
                self.hazard_sim.step(lap2_this_frame);
                self.hazard_sim.write_into(ram);
            }

            self.accumulator -= frame_duration;
            stepped += 1;
        }
        if stepped == MAX_CATCHUP_FRAMES {
            self.accumulator = Duration::ZERO;
        }

        self.fps_frame_count += stepped;
        let fps_elapsed = now.duration_since(self.fps_window_start);
        if fps_elapsed >= Duration::from_secs(1) {
            self.current_fps = self.fps_frame_count as f64 / fps_elapsed.as_secs_f64();
            self.fps_frame_count = 0;
            self.fps_window_start = now;
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
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
            return;
        }

        for (i, &argb) in self.framebuffer_argb.iter().enumerate() {
            let a = (argb >> 24) as u8;
            let o = i * 4;
            self.framebuffer_rgba8[o] = (argb >> 16) as u8;
            self.framebuffer_rgba8[o + 1] = (argb >> 8) as u8;
            self.framebuffer_rgba8[o + 2] = argb as u8;
            self.framebuffer_rgba8[o + 3] = if a == 0 { 255 } else { a };
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([fb_w, fb_h], &self.framebuffer_rgba8);
        match &mut self.texture {
            Some(tex) => tex.set(image, egui::TextureOptions::NEAREST),
            None => {
                self.texture = Some(ctx.load_texture("emulator_framebuffer", image, egui::TextureOptions::NEAREST));
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

        self.run_emulated_frames(p1, p2);

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

        self.update_texture(ctx);
        let (fb_w, fb_h) = self.frame_dims;

        let side_panel_width = (ctx.screen_rect().width() * 0.25).max(180.0);

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
            writer::inject_track_index(self.emu.system_ram_mut(), track_index);
            self.race_track_index = Some(track_index);
        }

        let joined_this_frame = lobby_events
            .iter()
            .any(|e| matches!(e, ServerEvent::Joined { .. } | ServerEvent::RoomCreated { .. }));

        let wram_requests = self.game_flow.poll(
            self.emu.system_ram_ref(),
            self.is_host,
            joined_this_frame,
            self.room_racing,
            self.lobby.start_track(),
        );
        for request in wram_requests {
            self.lobby.send(request);
        }

        if self.game_flow.left_room_fired {
            self.awaiting_title_before_rejoin = true;
        }
        if self.game_flow.at_title_screen {
            self.awaiting_title_before_rejoin = false;
        }
        self.lobby.set_can_join(!self.awaiting_title_before_rejoin);

        if self.game_flow.race_started_fired {
            writer::write_mailbox_bit(self.emu.system_ram_mut(), wram::ALL_READY_BIT, false);
        }

        if self.game_flow.entered_race_edge {
            self.handle_entered_race_edge();
        }

        let in_race = {
            let ram = self.emu.system_ram_ref();
            ram.get(wram::GAMEMODE_ADDR).copied() == Some(wram::GAMEMODE_IN_RACE)
        };
        if in_race {
            self.run_in_race_sync();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tex) = &self.texture {
                let avail = ui.available_size();
                let mut size = egui::vec2(fb_w as f32 * self.settings.scale, fb_h as f32 * self.settings.scale);
                if avail.x < size.x || avail.y < size.y {
                    let scale = (avail.x / size.x).min(avail.y / size.y);
                    size *= scale;
                }
                ui.add(egui::Image::new(tex).fit_to_exact_size(size));
            }
        });

        ctx.request_repaint_after(Duration::from_secs_f64(1.0 / TARGET_FPS));
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
    let core_path = locate_core_path(core_filename);
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