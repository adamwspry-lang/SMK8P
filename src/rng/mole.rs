use crate::rng::fish::RngSource;

pub const PEEK_FRAMES: i32 = 30;
pub const JUMP_VEL: i16 = 0xA0;
pub const JUMP_DECAY: i16 = 8;

pub const MOLE_DISPATCH_ORDER: [usize; 4] = [0, 3, 2, 1];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Wait,
    Peek,
    Launch,
    Jump,
}

impl Phase {
    pub fn state_code(self) -> u8 {
        match self {
            Phase::Idle => 0,
            Phase::Wait => 1,
            Phase::Peek => 2,
            Phase::Launch => 3,
            Phase::Jump => 4,
        }
    }
}

fn draw_n(rng: &mut dyn RngSource) -> i32 {
    ((rng.next() as i32) & 0x3F) + 0x10
}

#[derive(Clone, Copy)]
pub struct Mole {
    pub phase: Phase,
    pub cd: i32,
    pub pos: i16,
    pub vel: i16,
}

impl Mole {
    pub fn spawn() -> Mole {
        Mole { phase: Phase::Idle, cd: 0, pos: 0, vel: 0 }
    }

    pub fn step(&mut self, rng: &mut dyn RngSource) {
        match self.phase {
            Phase::Idle => {
                self.cd = draw_n(rng);
                self.pos = 0;
                self.vel = 0;
                self.phase = Phase::Wait;
            }
            Phase::Wait => {
                self.cd -= 1;
                if self.cd <= 0 {
                    self.phase = Phase::Peek;
                    self.cd = PEEK_FRAMES;
                }
            }
            Phase::Peek => {
                self.cd -= 1;
                if self.cd <= 0 {
                    self.phase = Phase::Launch;
                    self.cd = 4;
                }
            }
            Phase::Launch => {
                self.vel = JUMP_VEL;
                self.phase = Phase::Jump;
            }
            Phase::Jump => {
                self.pos = self.pos.wrapping_add(self.vel);
                self.vel = self.vel.wrapping_sub(JUMP_DECAY);
                if self.pos < 0 {
                    self.pos = 0;
                    self.phase = Phase::Idle;
                }
            }
        }
    }
}