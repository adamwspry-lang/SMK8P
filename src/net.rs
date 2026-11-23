use serde::{Deserialize, Serialize};
use socket2::{Domain, Socket, TcpKeepalive, Type};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

const KEEPALIVE_IDLE: Duration = Duration::from_secs(30);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const KEEPALIVE_RETRIES: u32 = 4;

#[derive(Debug, Serialize)]
struct OutMessage {
    #[serde(rename = "type")]
    msg_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_track: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item_type: Option<u8>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    alt_direction: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    evicted_owner_slot: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_slot: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tile_index: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tile_byte0: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tile_byte1: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync_item_boxes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync_coins: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync_rng: Option<bool>,
}

impl OutMessage {
    fn bare(msg_type: &'static str) -> Self {
        Self {
            msg_type,
            code: None,
            name: None,
            public: None,
            slot: None,
            start_track: None,
            item_type: None,
            alt_direction: false,
            evicted_owner_slot: None,
            owner_slot: None,
            tile_index: None,
            tile_byte0: None,
            tile_byte1: None,
            sync_item_boxes: None,
            sync_coins: None,
            sync_rng: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Request {
    CreateRoom { name: String, public: bool },
    JoinRoom { name: String, code: String },
    ListRooms,
    StartRoom { start_track: i32 },
    Ready,
    Kick { slot: i32 },
    LeaveRoom,
    Finished,
    ThrowItem { item_type: u8, alt_direction: bool, evicted_owner_slot: Option<i32> },
    ItemUsed { owner_slot: i32 },
    ItemDespawning { owner_slot: i32 },
    CollectibleTaken { tile_index: u16, tile_byte0: u8, tile_byte1: Option<u8> },
    EnteredLap2,
    LightningUsed,
    SetRoomOptions { sync_item_boxes: Option<bool>, sync_coins: Option<bool>, sync_rng: Option<bool> },
}

impl From<Request> for OutMessage {
    fn from(req: Request) -> Self {
        match req {
            Request::CreateRoom { name, public } => OutMessage {
                name: Some(name),
                public: Some(public),
                ..OutMessage::bare("create_room")
            },
            Request::JoinRoom { name, code } => OutMessage {
                name: Some(name),
                code: Some(code),
                ..OutMessage::bare("join_room")
            },
            Request::ListRooms => OutMessage::bare("list_rooms"),
            Request::StartRoom { start_track } => OutMessage {
                start_track: Some(start_track),
                ..OutMessage::bare("start_room")
            },
            Request::Ready => OutMessage::bare("ready"),
            Request::Kick { slot } => OutMessage {
                slot: Some(slot),
                ..OutMessage::bare("kick")
            },
            Request::LeaveRoom => OutMessage::bare("leave_room"),
            Request::Finished => OutMessage::bare("finished"),
            Request::ThrowItem { item_type, alt_direction, evicted_owner_slot } => OutMessage {
                item_type: Some(item_type),
                alt_direction,
                evicted_owner_slot,
                ..OutMessage::bare("throw_item")
            },
            Request::ItemUsed { owner_slot } => {
                OutMessage { owner_slot: Some(owner_slot), ..OutMessage::bare("item_used") }
            }
            Request::ItemDespawning { owner_slot } => {
                OutMessage { owner_slot: Some(owner_slot), ..OutMessage::bare("item_despawning") }
            }
            Request::CollectibleTaken { tile_index, tile_byte0, tile_byte1 } => OutMessage {
                tile_index: Some(tile_index),
                tile_byte0: Some(tile_byte0),
                tile_byte1,
                ..OutMessage::bare("collectible_taken")
            },
            Request::EnteredLap2 => OutMessage::bare("entered_lap2"),
            Request::LightningUsed => OutMessage::bare("lightning_used"),
            Request::SetRoomOptions { sync_item_boxes, sync_coins, sync_rng } => OutMessage {
                sync_item_boxes,
                sync_coins,
                sync_rng,
                ..OutMessage::bare("set_room_options")
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RoomCreated {
    code: String,
    start_pos: i32,
    #[serde(default)]
    udp_token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Joined {
    code: String,
    #[serde(default)]
    start_pos: i32,
    #[serde(default)]
    udp_token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct FoldedIn {
    start_pos: i32,
    #[serde(default)]
    next_track: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct RoomSummary {
    code: String,
    player_count: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct RoomList {
    #[serde(default)]
    rooms: Option<Vec<RoomSummary>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoomMember {
    pub name: String,
    pub slot: i32,
    pub start_pos: i32,
    #[serde(default)]
    pub ready: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RoomState {
    code: String,
    status: String,
    active: Vec<RoomMember>,
    pending: Vec<RoomMember>,
    is_host: Option<bool>,
    all_active_ready: bool,
    #[serde(default)]
    next_track: i32,
    #[serde(default)]
    next_rng_seed: i32,
    #[serde(default = "default_true")]
    sync_item_boxes: bool,
    #[serde(default = "default_true")]
    sync_coins: bool,
    #[serde(default = "default_true")]
    sync_rng: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
struct RaceStart {
    rng_seed: i32,
    slot: i32,
    start_pos: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ItemSpawnMsg {
    thrower_slot: i32,
    item_type: u8,
    #[serde(default)]
    alt_direction: bool,
    #[serde(default)]
    evicted_owner_slot: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ItemUsedMsg {
    owner_slot: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct LightningUsedMsg {
    thrower_slot: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct CollectibleTakenMsg {
    tile_index: u16,
    tile_byte0: u8,
    #[serde(default)]
    tile_byte1: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
struct RoomClosed {
    reason: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ErrorMsg {
    reason: String,
    message: String,
}

#[derive(Debug, Clone)]
pub enum ServerEvent {
    RoomCreated { code: String, start_pos: i32, udp_token: String },
    Joined { code: String, start_pos: i32, udp_token: String },
    FoldedIn { start_pos: i32, next_track: i32 },
    RoomList { rooms: Vec<(String, i32)> },
    RoomState {
        code: String,
        status: String,
        active: Vec<RoomMember>,
        pending: Vec<RoomMember>,
        is_host: Option<bool>,
        all_active_ready: bool,
        next_track: i32,
        next_rng_seed: i32,
        sync_item_boxes: bool,
        sync_coins: bool,
        sync_rng: bool,
    },
    RaceStart { rng_seed: i32, slot: i32, start_pos: Option<i32> },
    RoomClosed { reason: String },
    Kicked,
    Left,
    Error { reason: String, message: String },
    ItemSpawn { thrower_slot: i32, item_type: u8, alt_direction: bool, evicted_owner_slot: Option<i32> },
    ItemUsed { owner_slot: i32 },
    ItemDespawning { owner_slot: i32 },
    CollectibleTaken { tile_index: u16, tile_byte0: u8, tile_byte1: Option<u8> },
    Lap2Entered,
    LightningUsed { thrower_slot: i32 },
    Disconnected,
}

#[derive(Debug, Deserialize)]
struct TypeOnly {
    #[serde(rename = "type")]
    msg_type: String,
}

macro_rules! parse_or_log {
    ($ty:ty, $type_name:literal, $line:expr) => {
        match serde_json::from_str::<$ty>($line) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!(
                    "net: failed to parse {} message: {e} (line: {})",
                    $type_name, $line
                );
                None
            }
        }
    };
}

fn parse_event(line: &str) -> Option<ServerEvent> {
    let TypeOnly { msg_type } = match serde_json::from_str(line) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("net: couldn't even read a \"type\" field from server line: {e} (line: {line})");
            return None;
        }
    };
    match msg_type.as_str() {
        "room_created" => {
            let m: RoomCreated = parse_or_log!(RoomCreated, "room_created", line)?;
            Some(ServerEvent::RoomCreated { code: m.code, start_pos: m.start_pos, udp_token: m.udp_token })
        }
        "joined" => {
            let m: Joined = parse_or_log!(Joined, "joined", line)?;
            Some(ServerEvent::Joined { code: m.code, start_pos: m.start_pos, udp_token: m.udp_token })
        }
        "folded_in" => {
            let m: FoldedIn = parse_or_log!(FoldedIn, "folded_in", line)?;
            Some(ServerEvent::FoldedIn { start_pos: m.start_pos, next_track: m.next_track })
        }
        "room_list" => {
            let m: RoomList = parse_or_log!(RoomList, "room_list", line)?;
            Some(ServerEvent::RoomList {
                rooms: m.rooms.unwrap_or_default().into_iter().map(|r| (r.code, r.player_count)).collect(),
            })
        }
        "room_state" => {
            let m: RoomState = parse_or_log!(RoomState, "room_state", line)?;
            Some(ServerEvent::RoomState {
                code: m.code,
                status: m.status,
                active: m.active,
                pending: m.pending,
                is_host: m.is_host,
                all_active_ready: m.all_active_ready,
                next_track: m.next_track,
                next_rng_seed: m.next_rng_seed,
                sync_item_boxes: m.sync_item_boxes,
                sync_coins: m.sync_coins,
                sync_rng: m.sync_rng,
            })
        }
        "race_start" => {
            let m: RaceStart = parse_or_log!(RaceStart, "race_start", line)?;
            Some(ServerEvent::RaceStart { rng_seed: m.rng_seed, slot: m.slot, start_pos: m.start_pos })
        }
        "room_closed" => {
            let m: RoomClosed = parse_or_log!(RoomClosed, "room_closed", line)?;
            Some(ServerEvent::RoomClosed { reason: m.reason })
        }
        "item_spawn" => {
            let m: ItemSpawnMsg = parse_or_log!(ItemSpawnMsg, "item_spawn", line)?;
            Some(ServerEvent::ItemSpawn {
                thrower_slot: m.thrower_slot,
                item_type: m.item_type,
                alt_direction: m.alt_direction,
                evicted_owner_slot: m.evicted_owner_slot,
            })
        }
        "item_used" => {
            let m: ItemUsedMsg = parse_or_log!(ItemUsedMsg, "item_used", line)?;
            Some(ServerEvent::ItemUsed { owner_slot: m.owner_slot })
        }
        "item_despawning" => {
            let m: ItemUsedMsg = parse_or_log!(ItemUsedMsg, "item_despawning", line)?;
            Some(ServerEvent::ItemDespawning { owner_slot: m.owner_slot })
        }
        "collectible_taken" => {
            let m: CollectibleTakenMsg = parse_or_log!(CollectibleTakenMsg, "collectible_taken", line)?;
            Some(ServerEvent::CollectibleTaken {
                tile_index: m.tile_index,
                tile_byte0: m.tile_byte0,
                tile_byte1: m.tile_byte1,
            })
        }
        "kicked" => Some(ServerEvent::Kicked),
        "left" => Some(ServerEvent::Left),
        "lap2_entered" => Some(ServerEvent::Lap2Entered),
        "lightning_used" => {
            let m: LightningUsedMsg = parse_or_log!(LightningUsedMsg, "lightning_used", line)?;
            Some(ServerEvent::LightningUsed { thrower_slot: m.thrower_slot })
        }
        "error" => {
            let m: ErrorMsg = parse_or_log!(ErrorMsg, "error", line)?;
            Some(ServerEvent::Error { reason: m.reason, message: m.message })
        }
        other => {
            eprintln!("net: unrecognized message type from server: {other:?} (line: {line})");
            None
        }
    }
}

pub struct LobbyClient {
    stream: TcpStream,
    events: Receiver<ServerEvent>,
}

impl LobbyClient {
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let sock_addr: SocketAddr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no addresses resolved"))?;

        let socket = Socket::new(Domain::for_address(sock_addr), Type::STREAM, None)?;
        socket.connect(&sock_addr.into())?;

        let keepalive = TcpKeepalive::new()
            .with_time(KEEPALIVE_IDLE)
            .with_interval(KEEPALIVE_INTERVAL);
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
        let keepalive = keepalive.with_retries(KEEPALIVE_RETRIES);
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
        let _ = KEEPALIVE_RETRIES;
        if let Err(e) = socket.set_tcp_keepalive(&keepalive) {
            eprintln!("net: couldn't enable TCP keepalive (continuing without it): {e}");
        }

        let stream: TcpStream = socket.into();
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        stream.set_nodelay(true)?;
        let reader_stream = stream.try_clone()?;

        let (tx, rx): (Sender<ServerEvent>, Receiver<ServerEvent>) = mpsc::channel();
        thread::spawn(move || read_loop(reader_stream, tx));

        Ok(Self { stream, events: rx })
    }

    pub fn send(&mut self, request: Request) -> std::io::Result<()> {
        let out: OutMessage = request.into();
        let mut line = serde_json::to_string(&out).expect("OutMessage always serializes");
        line.push('\n');

        match self.stream.write_all(line.as_bytes()) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("net: WRITE FAILED for {}: {e} (kind: {:?})", line.trim_end(), e.kind());
                Err(e)
            }
        }
    }

    pub fn poll_events(&self) -> Vec<ServerEvent> {
        self.events.try_iter().collect()
    }
}

fn read_loop(stream: TcpStream, tx: Sender<ServerEvent>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = tx.send(ServerEvent::Disconnected);
                return;
            }
            Ok(_) => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(event) = parse_event(trimmed) {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("net: read error, treating as disconnect: {e}");
                let _ = tx.send(ServerEvent::Disconnected);
                return;
            }
        }
    }
}