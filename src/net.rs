use serde::{Deserialize, Serialize};
use socket2::{Domain, Socket, TcpKeepalive, Type};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

// Read side has no app-level timeout today; the read loop just blocks on
// read_line(). Keepalive is what makes it wake up with a Disconnected event
// instead of sitting there forever on a half-dead connection.
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
    position: Option<i32>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    start_track: Option<i32>,
}

impl OutMessage {
    fn bare(msg_type: &'static str) -> Self {
        Self {
            msg_type,
            code: None,
            name: None,
            public: None,
            slot: None,
            position: None,
            start_track: None,
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
    
    Finished { position: i32 },
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
            Request::Finished { position } => OutMessage {
                position: Some(position),
                ..OutMessage::bare("finished")
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
}

#[derive(Debug, Clone, Deserialize)]
struct RaceStart {
    rng_seed: i32,
    slot: i32,
    
    start_pos: Option<i32>,
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
    },
    RaceStart { rng_seed: i32, slot: i32, start_pos: Option<i32> },
    RoomClosed { reason: String },
    Kicked,
    Left,
    Error { reason: String, message: String },
    
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
        "kicked" => Some(ServerEvent::Kicked),
        "left" => Some(ServerEvent::Left),
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
            Ok(()) => {
                eprintln!("net: sent {} bytes: {}", line.len(), line.trim_end());
                Ok(())
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_room_serializes_like_main_go_expects() {
        let out: OutMessage = Request::CreateRoom { name: "ADM".into(), public: true }.into();
        let json = serde_json::to_string(&out).unwrap();
        assert!(json.contains(r#""type":"create_room""#));
        assert!(json.contains(r#""name":"ADM""#));
        assert!(json.contains(r#""public":true"#));
        
        assert!(!json.contains("code"));
        assert!(!json.contains("slot"));
    }

    #[test]
    fn leave_room_has_no_extra_fields() {
        let out: OutMessage = Request::LeaveRoom.into();
        let json = serde_json::to_string(&out).unwrap();
        assert_eq!(json, r#"{"type":"leave_room"}"#);
    }

    #[test]
    fn finished_serializes_like_main_go_expects() {
        let out: OutMessage = Request::Finished { position: 3 }.into();
        let json = serde_json::to_string(&out).unwrap();
        assert_eq!(json, r#"{"type":"finished","position":3}"#);
    }

    #[test]
    fn parses_room_state_with_is_host_present() {
        let line = r#"{"type":"room_state","code":"ADM","status":"lobby","active":[{"name":"ADM","slot":0,"start_pos":1}],"pending":[],"is_host":true,"all_active_ready":false}"#;
        match parse_event(line) {
            Some(ServerEvent::RoomState { code, status, active, is_host, all_active_ready, .. }) => {
                assert_eq!(code, "ADM");
                assert_eq!(status, "lobby");
                assert_eq!(active.len(), 1);
                assert_eq!(active[0].name, "ADM");
                assert_eq!(active[0].slot, 0);
                assert_eq!(is_host, Some(true));
                assert_eq!(all_active_ready, false);
            }
            other => panic!("expected RoomState, got {other:?}"),
        }
    }

    #[test]
    fn parses_room_state_with_is_host_absent_once_racing() {
        
        let line = r#"{"type":"room_state","code":"ADM","status":"racing","active":[{"name":"ADM","slot":0,"start_pos":1}],"pending":[],"all_active_ready":true}"#;
        match parse_event(line) {
            Some(ServerEvent::RoomState { is_host, status, all_active_ready, .. }) => {
                assert_eq!(status, "racing");
                assert_eq!(is_host, None);
                assert_eq!(all_active_ready, true);
            }
            other => panic!("expected RoomState, got {other:?}"),
        }
    }

    #[test]
    fn parses_pending_member_with_unassigned_slot() {
        
        let line = r#"{"type":"room_state","code":"ADM","status":"racing","active":[{"name":"ADM","slot":0,"start_pos":1}],"pending":[{"name":"BOB","slot":-1,"start_pos":2}],"all_active_ready":true}"#;
        match parse_event(line) {
            Some(ServerEvent::RoomState { pending, .. }) => {
                assert_eq!(pending.len(), 1);
                assert_eq!(pending[0].slot, -1);
            }
            other => panic!("expected RoomState, got {other:?}"),
        }
    }

    #[test]
    fn parses_joined_without_start_pos_as_pending() {
        
        let line = r#"{"type":"joined","code":"ADM"}"#;
        match parse_event(line) {
            Some(ServerEvent::Joined { code, start_pos, .. }) => {
                assert_eq!(code, "ADM");
                assert_eq!(start_pos, 0);
            }
            other => panic!("expected Joined, got {other:?}"),
        }
    }

    #[test]
    fn parses_folded_in() {
        let line = r#"{"type":"folded_in","start_pos":4,"next_track":7}"#;
        match parse_event(line) {
            Some(ServerEvent::FoldedIn { start_pos, next_track }) => {
                assert_eq!(start_pos, 4);
                assert_eq!(next_track, 7);
            }
            other => panic!("expected FoldedIn, got {other:?}"),
        }
    }

    #[test]
    fn parses_race_start_without_start_pos() {
        let line = r#"{"type":"race_start","rng_seed":4242,"slot":2}"#;
        match parse_event(line) {
            Some(ServerEvent::RaceStart { rng_seed, slot, start_pos }) => {
                assert_eq!(rng_seed, 4242);
                assert_eq!(slot, 2);
                assert_eq!(start_pos, None);
            }
            other => panic!("expected RaceStart, got {other:?}"),
        }
    }

    #[test]
    fn parses_error_message() {
        let line = r#"{"type":"error","reason":"not_host","message":"only the room host can start the room"}"#;
        match parse_event(line) {
            Some(ServerEvent::Error { reason, message }) => {
                assert_eq!(reason, "not_host");
                assert_eq!(message, "only the room host can start the room");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parses_room_list_null_as_empty() {
        let line = r#"{"type":"room_list","rooms":null}"#;
        match parse_event(line) {
            Some(ServerEvent::RoomList { rooms }) => assert!(rooms.is_empty()),
            other => panic!("expected RoomList, got {other:?}"),
        }
    }

    #[test]
    fn unrecognized_type_is_ignored_not_fatal() {
        let line = r#"{"type":"something_future_versions_added"}"#;
        assert!(parse_event(line).is_none());
    }
}