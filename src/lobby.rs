use crate::net::{LobbyClient, Request, RoomMember, ServerEvent};
use eframe::egui;
                   
const HOST_COLOR: egui::Color32 = egui::Color32::from_rgb(0, 153, 0);

const PENDING_COLOR: egui::Color32 = egui::Color32::from_rgb(51, 102, 255);

const READY_COLOR: egui::Color32 = egui::Color32::from_rgb(230, 200, 0);

#[derive(Debug, Clone, PartialEq)]
enum PlayerStatus {
    Host,
    Active,
    Pending,
    Ready,
}

#[derive(Debug, Clone)]
struct RosterRow {
    name: String,
    
    slot: i32,
    status: PlayerStatus,
}

enum Screen {
    NotInRoom,
    InRoom,
}

pub struct LobbyApp {
    client: Option<LobbyClient>,

    screen: Screen,

    name_input: String,
    join_code_input: String,
    create_public: bool,
    public_rooms: Vec<(String, i32)>,
    last_refresh: std::time::Instant,
    refresh_interval: std::time::Duration,

    room_code: String,
    roster: Vec<RosterRow>,
    
    status: String,
    
    is_host: bool,

    last_error: Option<String>,

    server_addr: String,

    start_track_input: u8,

    show_mushroom_prompt: bool,

    received_first_race_start: bool,

    settings_clicked: bool,
}

impl LobbyApp {
    pub fn new(server_addr: String) -> Self {
        Self {
            client: None,
            screen: Screen::NotInRoom,
            name_input: String::new(),
            join_code_input: String::new(),
            create_public: true,
            public_rooms: Vec::new(),
            last_refresh: std::time::Instant::now(),
            refresh_interval: std::time::Duration::from_secs(4),
            room_code: String::new(),
            roster: Vec::new(),
            status: String::new(),
            is_host: false,
            last_error: None,
            server_addr,
            start_track_input: 0,
            show_mushroom_prompt: false,
            received_first_race_start: false,
            settings_clicked: false,
        }
    }

    pub fn set_mushroom_prompt(&mut self, can_enter: bool) {
        self.show_mushroom_prompt = can_enter;
    }

    pub fn take_settings_clicked(&mut self) -> bool {
        std::mem::take(&mut self.settings_clicked)
    }

    pub fn start_track(&self) -> u8 {
        self.start_track_input
    }

    fn ensure_connected(&mut self) {
        if self.client.is_none() {
            match LobbyClient::connect(&self.server_addr) {
                Ok(c) => self.client = Some(c),
                Err(e) => {
                    self.last_error = Some(format!("couldn't connect to {}: {e}", self.server_addr));
                }
            }
        }
    }

    pub fn send(&mut self, request: Request) {
        self.ensure_connected();
        if let Some(client) = &mut self.client {
            if let Err(e) = client.send(request) {
                self.last_error = Some(format!("send failed: {e}"));
                self.client = None;
                self.return_to_screen1();
            }
        }
    }

    fn poll_events(&mut self) -> Vec<ServerEvent> {
        let Some(client) = &self.client else { return Vec::new() };
        client.poll_events()
    }

    fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::RoomCreated { code, .. } | ServerEvent::Joined { code, .. } => {
                self.room_code = code;
                self.screen = Screen::InRoom;
                self.last_error = None;
            }
            ServerEvent::RoomList { rooms } => {
                self.public_rooms = rooms;
            }
            
            ServerEvent::FoldedIn { .. } => {}
            ServerEvent::RoomState { code, status, active, pending, is_host, all_active_ready, next_track: _ } => {
                
                self.room_code = code;
                self.status = status.clone();
                
                if let Some(host) = is_host {
                    self.is_host = host;
                }
                self.roster = build_roster(&status, &active, &pending, is_host, all_active_ready);
            }
            ServerEvent::RoomClosed { reason } => {
                self.last_error = Some(format!("room closed: {reason}"));
                self.return_to_screen1();
            }
            ServerEvent::Kicked => {
                self.last_error = Some("you were kicked from the room".to_string());
                self.return_to_screen1();
            }
            ServerEvent::Left => {
                self.return_to_screen1();
            }
            ServerEvent::Error { reason, message } => {
                self.last_error = Some(format!("{reason}: {message}"));
            }
            ServerEvent::Disconnected => {
                self.last_error = Some("disconnected from server".to_string());
                self.client = None;
                self.return_to_screen1();
            }
            
            ServerEvent::RaceStart { .. } => {
                self.received_first_race_start = true;
            }
        }
    }

    fn return_to_screen1(&mut self) {
        self.screen = Screen::NotInRoom;
        self.room_code.clear();
        self.roster.clear();
        self.status.clear();
        self.is_host = false;
        
        self.received_first_race_start = false;
    }
}

fn build_roster(
    status: &str,
    active: &[RoomMember],
    pending: &[RoomMember],
    is_host: Option<bool>,
    all_active_ready: bool,
) -> Vec<RosterRow> {
    let mut rows = Vec::new();
    for (i, member) in active.iter().enumerate() {
        
        let status_for_row = if status == "lobby" && i == 0 {
            PlayerStatus::Host
        } else if member.ready && !all_active_ready {
            PlayerStatus::Ready
        } else {
            PlayerStatus::Active
        };
        let _ = is_host;
        rows.push(RosterRow { name: member.name.clone(), slot: member.slot, status: status_for_row });
    }
    for member in pending {
        
        rows.push(RosterRow { name: member.name.clone(), slot: member.slot, status: PlayerStatus::Pending });
    }
    rows
}

impl LobbyApp {
    
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Vec<ServerEvent> {
        let events = self.poll_events();
        for event in &events {
            self.handle_event(event.clone());
        }

        if let Some(err) = &self.last_error {
            ui.colored_label(egui::Color32::RED, err);
            ui.separator();
        }

        match self.screen {
            Screen::NotInRoom => self.draw_screen1(ui),
            Screen::InRoom => self.draw_screen2(ui),
        }

        events
    }
}

impl LobbyApp {
    
    fn draw_screen1(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("SMK 8-Player");
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    self.settings_clicked = true;
                }
            });
        });
        ui.separator();

        ui.label("Name (3 letters):");
        ui.text_edit_singleline(&mut self.name_input);
        
        self.name_input = self.name_input.to_uppercase();
        self.name_input.truncate(3);

        let name_valid = valid_name(&self.name_input);
        if !name_valid {
            ui.colored_label(egui::Color32::GRAY, "Enter a 3-letter name to continue");
        }

        ui.separator();

        ui.add_enabled_ui(name_valid, |ui| {
            
            ui.label("Create Room");
            ui.checkbox(&mut self.create_public, "Public room");
            if ui.button("Create Room").clicked() {
                self.send(Request::CreateRoom {
                    name: self.name_input.clone(),
                    public: self.create_public,
                });
            }

            ui.separator();

            ui.label("Join Room");
            ui.text_edit_singleline(&mut self.join_code_input);
            self.join_code_input = self.join_code_input.to_uppercase();
            let code_valid = !self.join_code_input.is_empty();
            if ui
                .add_enabled(code_valid, egui::Button::new("Join Room"))
                .clicked()
            {
                self.send(Request::JoinRoom {
                    name: self.name_input.clone(),
                    code: self.join_code_input.clone(),
                });
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Public Rooms");
                if ui.small_button("Refresh").clicked() {
                    self.send(Request::ListRooms);
                    self.last_refresh = std::time::Instant::now();
                }
            });
            if self.last_refresh.elapsed() >= self.refresh_interval {
                self.send(Request::ListRooms);
                self.last_refresh = std::time::Instant::now();
            }

            let mut clicked_code: Option<String> = None;
            for (code, count) in &self.public_rooms {
                if ui.button(format!("{code}  ({count}/8)")).clicked() {
                    clicked_code = Some(code.clone());
                }
            }
            if let Some(code) = clicked_code {
                self.join_code_input = code.clone();
                self.send(Request::JoinRoom { name: self.name_input.clone(), code });
            }
        });
    }

    fn draw_screen2(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(format!("Room: {}", self.room_code));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    self.settings_clicked = true;
                }
            });
        });
        ui.separator();

        if self.show_mushroom_prompt && !self.received_first_race_start {
            ui.colored_label(READY_COLOR, "Please enter the Mushroom Cup");
            ui.separator();
        }

        if self.is_host && self.status == "lobby" {
            ui.horizontal(|ui| {
                ui.label("Start track:");
                let mut value = self.start_track_input as i32;
                if ui
                    .add(egui::DragValue::new(&mut value).range(0..=19))
                    .changed()
                {
                    self.start_track_input = value as u8;
                }
            });
            ui.separator();
        }

        ui.label(format!("{}/8 players", self.roster.len()));
        ui.separator();

        let show_kick = self.is_host && self.status == "lobby";
        let mut kick_slot: Option<i32> = None;

        for row in &self.roster {
            let text = match row.status {
                PlayerStatus::Host => egui::RichText::new(&row.name).color(HOST_COLOR),
                PlayerStatus::Pending => egui::RichText::new(&row.name).color(PENDING_COLOR),
                PlayerStatus::Ready => egui::RichText::new(&row.name).color(READY_COLOR),
                PlayerStatus::Active => egui::RichText::new(&row.name),
            };
            ui.horizontal(|ui| {
                ui.label(text);
                
                let is_self = row.name == self.name_input;
                let kickable = show_kick && !is_self && row.status != PlayerStatus::Pending;
                if kickable && ui.small_button("Kick").clicked() {
                    kick_slot = Some(row.slot);
                }
            });
        }
        if let Some(slot) = kick_slot {
            self.send(Request::Kick { slot });
        }

        ui.separator();

        if self.status != "racing" {
            if ui.button("Exit Room").clicked() {
                self.send(Request::LeaveRoom);
                self.return_to_screen1();
            }
        }
    }
}

fn valid_name(name: &str) -> bool {
    name.len() == 3 && name.chars().all(|c| c.is_ascii_alphabetic())
}