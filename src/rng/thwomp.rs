use crate::rng::fish::RngSource;

pub const JIGGLE_FRAMES: i32 = 14;
pub const GROUND_FRAMES: i32 = 120;
pub const RISE_FRAMES: i32 = 65;

pub const BC2_DISPATCH_ORDER: [usize; 4] = [0, 3, 2, 1];

fn draw_n(rng: &mut dyn RngSource) -> i32 {
    ((rng.next() as i32) & 0x7F) + 0x10
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Trigger,
    Countdown,
    Fall,
    Jiggle,
    GroundWait,
    Rise,
}

impl Phase {
    pub fn state_code(self) -> u8 {
        match self {
            Phase::Idle => 0,
            Phase::Trigger => 0,
            Phase::Countdown => 1,
            Phase::Fall => 2,
            Phase::Jiggle => 3,
            Phase::GroundWait => 4,
            Phase::Rise => 5,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Thwomp {
    pub phase: Phase,
    pub remaining: i32,
    pub pos: i16,
    pub frac: u8,
    pub vel: i16,
    activated: bool,
}

impl Thwomp {
    pub fn spawn() -> Thwomp {
        Thwomp {
            phase: Phase::Idle,
            remaining: 0,
            pos: 0x10,
            frac: 0,
            vel: 0,
            activated: false,
        }
    }

    fn integrate(&mut self) {
        let acc = (((self.pos as i32) << 8) | (self.frac as i32)).wrapping_add(self.vel as i32);
        self.frac = (acc & 0xFF) as u8;
        self.pos = (acc >> 8) as i16;
    }

    pub fn step(&mut self, rng: &mut dyn RngSource, activate_now: bool) {
        if !self.activated {
            if activate_now {
                self.activated = true;
                let n = draw_n(rng);
                self.phase = Phase::Countdown;
                self.remaining = n;
            }
            return;
        }

        match self.phase {
            Phase::Idle => {}
            Phase::Trigger => {
                self.integrate();
                let n = draw_n(rng);
                self.phase = Phase::Countdown;
                self.remaining = n;
            }
            Phase::Countdown => {
                self.integrate();
                self.remaining -= 1;
                if self.remaining <= 0 {
                    self.phase = Phase::Fall;
                    self.vel = -0x40;
                }
            }
            Phase::Fall => {
                self.integrate();
                self.vel = self.vel.wrapping_sub(0x20);
                if self.pos < 0 {
                    self.pos = 0;
                    self.frac = 0;
                    self.vel = 0;
                    self.phase = Phase::Jiggle;
                    self.remaining = JIGGLE_FRAMES;
                }
            }
            Phase::Jiggle => {
                self.remaining -= 1;
                if self.remaining <= 0 {
                    self.phase = Phase::GroundWait;
                    self.remaining = GROUND_FRAMES;
                }
            }
            Phase::GroundWait => {
                self.remaining -= 1;
                if self.remaining <= 0 {
                    self.phase = Phase::Rise;
                    self.remaining = RISE_FRAMES;
                }
            }
            Phase::Rise => {
                self.integrate();
                self.vel = 0x40;
                self.remaining -= 1;
                if self.remaining <= 0 {
                    self.phase = Phase::Trigger;
                }
            }
        }
    }
}