use eframe::egui;
use gilrs::{Axis, Button, Gilrs};
use retro_rs::Buttons;
use serde::{Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.json";

const AXIS_THRESHOLD: f32 = 0.5;

const ALL_AXES: &[Axis] = &[
    Axis::LeftStickX,
    Axis::LeftStickY,
    Axis::RightStickX,
    Axis::RightStickY,
    Axis::LeftZ,
    Axis::RightZ,
    Axis::DPadX,
    Axis::DPadY,
];

const ALL_BUTTONS: &[Button] = &[
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::C,
    Button::Z,
    Button::LeftTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogicalButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L,
    R,
    Start,
    Select,
}

impl LogicalButton {
    const ALL: [LogicalButton; 12] = [
        LogicalButton::Up,
        LogicalButton::Down,
        LogicalButton::Left,
        LogicalButton::Right,
        LogicalButton::A,
        LogicalButton::B,
        LogicalButton::X,
        LogicalButton::Y,
        LogicalButton::L,
        LogicalButton::R,
        LogicalButton::Start,
        LogicalButton::Select,
    ];

    fn label(self) -> &'static str {
        match self {
            LogicalButton::Up => "Up",
            LogicalButton::Down => "Down",
            LogicalButton::Left => "Left",
            LogicalButton::Right => "Right",
            LogicalButton::A => "A",
            LogicalButton::B => "B",
            LogicalButton::X => "X",
            LogicalButton::Y => "Y",
            LogicalButton::L => "L",
            LogicalButton::R => "R",
            LogicalButton::Start => "Start",
            LogicalButton::Select => "Select",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InputSource {
    Keyboard(egui::Key),
    GamepadButton(Button),
    
    GamepadAxis { axis: Axis, positive: bool },
}

impl InputSource {
    fn describe(self) -> String {
        match self {
            InputSource::Keyboard(key) => format!("{key:?}"),
            InputSource::GamepadButton(b) => format!("Gamepad {b:?}"),
            InputSource::GamepadAxis { axis, positive } => {
                format!("Gamepad {axis:?} {}", if positive { "+" } else { "-" })
            }
        }
    }

    fn is_active(self, ctx: &egui::Context, gilrs: Option<&Gilrs>) -> bool {
        match self {
            InputSource::Keyboard(key) => ctx.input(|i| i.key_down(key)),
            InputSource::GamepadButton(button) => {
                let Some(gilrs) = gilrs else { return false };
                gilrs.gamepads().any(|(_, pad)| pad.is_pressed(button))
            }
            InputSource::GamepadAxis { axis, positive } => {
                let Some(gilrs) = gilrs else { return false };
                gilrs.gamepads().any(|(_, pad)| {
                    pad.axis_data(axis)
                        .map(|d| if positive { d.value() > AXIS_THRESHOLD } else { d.value() < -AXIS_THRESHOLD })
                        .unwrap_or(false)
                })
            }
        }
    }
}

fn poll_any_input(ctx: &egui::Context, gilrs: Option<&Gilrs>) -> Option<InputSource> {
    let key = ctx.input(|i| i.keys_down.iter().next().copied());
    if let Some(key) = key {
        return Some(InputSource::Keyboard(key));
    }
    let Some(gilrs) = gilrs else { return None };
    for (_, pad) in gilrs.gamepads() {
        for &button in ALL_BUTTONS {
            if pad.is_pressed(button) {
                return Some(InputSource::GamepadButton(button));
            }
        }
        for &axis in ALL_AXES {
            if let Some(data) = pad.axis_data(axis) {
                let v = data.value();
                if v > AXIS_THRESHOLD {
                    return Some(InputSource::GamepadAxis { axis, positive: true });
                }
                if v < -AXIS_THRESHOLD {
                    return Some(InputSource::GamepadAxis { axis, positive: false });
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputBindings {
    map: std::collections::HashMap<LogicalButton, InputSource>,
}

impl Default for InputBindings {
    
    fn default() -> Self {
        use egui::Key;
        use LogicalButton::*;
        let mut map = std::collections::HashMap::new();
        map.insert(Up, InputSource::Keyboard(Key::ArrowUp));
        map.insert(Down, InputSource::Keyboard(Key::ArrowDown));
        map.insert(Left, InputSource::Keyboard(Key::ArrowLeft));
        map.insert(Right, InputSource::Keyboard(Key::ArrowRight));
        map.insert(A, InputSource::Keyboard(Key::X));
        map.insert(B, InputSource::Keyboard(Key::Z));
        map.insert(X, InputSource::Keyboard(Key::S));
        map.insert(Y, InputSource::Keyboard(Key::A));
        map.insert(L, InputSource::Keyboard(Key::Q));
        map.insert(R, InputSource::Keyboard(Key::W));
        map.insert(Start, InputSource::Keyboard(Key::Enter));
        
        map.insert(Select, InputSource::Keyboard(Key::Backslash));
        Self { map }
    }
}

impl InputBindings {
    pub fn get(&self, button: LogicalButton) -> Option<InputSource> {
        self.map.get(&button).copied()
    }

    pub fn set(&mut self, button: LogicalButton, source: InputSource) {
        self.map.insert(button, source);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    
    pub volume: f32,
    pub bindings: InputBindings,
}

impl Default for Settings {
    fn default() -> Self {
        Self { volume: 1.0, bindings: InputBindings::default() }
    }
}

impl Settings {

    pub fn load() -> Self {
        let path = crate::exe_dir().join(SETTINGS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => settings,
                Err(e) => {
                    eprintln!("{path:?} failed to parse ({e}), using defaults");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = crate::exe_dir().join(SETTINGS_FILE);
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    eprintln!("couldn't save {path:?}: {e}");
                }
            }
            Err(e) => eprintln!("couldn't serialize settings: {e}"),
        }
    }

    pub fn read_buttons(&self, ctx: &egui::Context, gilrs: Option<&Gilrs>) -> Buttons {
        let pressed = |button: LogicalButton| {
            self.bindings
                .get(button)
                .map(|src| src.is_active(ctx, gilrs))
                .unwrap_or(false)
        };
        Buttons::new()
            .up(pressed(LogicalButton::Up))
            .down(pressed(LogicalButton::Down))
            .left(pressed(LogicalButton::Left))
            .right(pressed(LogicalButton::Right))
            .a(pressed(LogicalButton::A))
            .b(pressed(LogicalButton::B))
            .x(pressed(LogicalButton::X))
            .y(pressed(LogicalButton::Y))
            .l1(pressed(LogicalButton::L))
            .r1(pressed(LogicalButton::R))
            .start(pressed(LogicalButton::Start))
            .select(pressed(LogicalButton::Select))
    }
}

pub struct SettingsWindow {
    pub open: bool,
    
    rebinding: Option<LogicalButton>,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self { open: false, rebinding: None }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if !self.open {
            self.rebinding = None;
        }
    }

    pub fn draw(&mut self, ctx: &egui::Context, settings: &mut Settings, gilrs: Option<&Gilrs>) {
        if !self.open {
            return;
        }

        if let Some(target) = self.rebinding {
            if let Some(source) = poll_any_input(ctx, gilrs) {
                settings.bindings.set(target, source);
                self.rebinding = None;
            }
        }

        let mut open = self.open;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Audio");
                    ui.add(egui::Slider::new(&mut settings.volume, 0.0..=1.0).text("Volume"));

                    ui.separator();
                    ui.heading("Input Mapping (Player 1)");
                    ui.label("Click Rebind, then press any keyboard key or gamepad button/stick.");
                    ui.add_space(4.0);

                    egui::Grid::new("input_mapping_grid")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            for button in LogicalButton::ALL {
                                ui.label(button.label());
                                let bound_text = settings
                                    .bindings
                                    .get(button)
                                    .map(|s| s.describe())
                                    .unwrap_or_else(|| "(unbound)".to_string());
                                if self.rebinding == Some(button) {
                                    ui.colored_label(egui::Color32::YELLOW, "Press anything...");
                                } else {
                                    ui.label(bound_text);
                                }
                                let rebind_label = if self.rebinding == Some(button) { "Cancel" } else { "Rebind" };
                                if ui.button(rebind_label).clicked() {
                                    self.rebinding = if self.rebinding == Some(button) { None } else { Some(button) };
                                }
                                ui.end_row();
                            }
                        });

                    ui.separator();
                    if ui.button("Reset to defaults").clicked() {
                        settings.bindings = InputBindings::default();
                    }

                });
            });
        self.open = open;
    }
}