pub trait RngSource {
    fn next(&mut self) -> u16;
}

fn e05a(rng: &mut dyn RngSource) -> i16 {
    (rng.next() & 0x7F) as i16
}

fn e062(rng: &mut dyn RngSource) -> i16 {
    let raw = (rng.next() & 0x7F) as i16;
    raw.wrapping_neg()
}

fn e04f(rng: &mut dyn RngSource) -> i16 {
    let raw7 = rng.next() & 0x7F;
    let carry = raw7 & 1;
    let magnitude = (raw7 >> 1) as i16;
    if carry == 1 {
        magnitude
    } else {
        magnitude.wrapping_neg()
    }
}

fn axis_decision(diff: i16, rng: &mut dyn RngSource) -> i16 {
    if diff >= 0 {
        if diff < 0x20 {
            e04f(rng)
        } else {
            e062(rng)
        }
    } else {
        if diff >= -0x20 {
            e04f(rng)
        } else {
            e05a(rng)
        }
    }
}

#[derive(Clone, Copy)]
pub struct Fish {
    pub pos0: i16,
    pub vel0: i16,
    pub pos1: i16,
    pub vel1: i16,
    pub pos2: i16,
    pub vel2: i16,
    pub tgt1: i16,
    pub tgt2: i16,
    pub frac1: u8,
    pub frac2: u8,
}

impl Fish {
    pub fn spawn(tgt1: i16, tgt2: i16) -> Fish {
        Fish {
            pos0: 0,
            vel0: 0,
            pos1: tgt1,
            vel1: 0,
            pos2: tgt2,
            vel2: 0,
            tgt1,
            tgt2,
            frac1: 0,
            frac2: 0,
        }
    }

    pub fn step(&mut self, rng: &mut dyn RngSource) {
        let relaunching = self.pos0 < 0;

        self.vel0 = self.vel0.wrapping_sub(0x12);
        if relaunching {
            let raw = rng.next();
            self.vel0 = ((raw & 0x00FF) as i16).wrapping_add(0x40);
        }
        self.pos0 = self.pos0.wrapping_add(self.vel0);

        if relaunching {
            let diff1 = self.pos1.wrapping_sub(self.tgt1);
            self.vel1 = axis_decision(diff1, rng);

            let diff2 = self.pos2.wrapping_sub(self.tgt2);
            self.vel2 = axis_decision(diff2, rng);
        }

        let acc1 =
            (((self.pos1 as i32) << 8) | (self.frac1 as i32)).wrapping_add(self.vel1 as i32);
        self.frac1 = (acc1 & 0xFF) as u8;
        self.pos1 = (acc1 >> 8) as i16;

        let acc2 =
            (((self.pos2 as i32) << 8) | (self.frac2 as i32)).wrapping_add(self.vel2 as i32);
        self.frac2 = (acc2 & 0xFF) as u8;
        self.pos2 = (acc2 >> 8) as i16;
    }
}

pub const KB2_TARGETS: [(i16, i16); 4] = [
    (148, 500),
    (132, 748),
    (676, 828),
    (180, 380),
];

pub const KB2_DISPATCH_ORDER: [usize; 4] = [0, 3, 2, 1];

pub const KB1_TARGETS: [(i16, i16); 4] = [
    (676, 332),
    (820, 508),
    (700, 756),
    (148, 740),
];

pub const KB1_DISPATCH_ORDER: [usize; 4] = [0, 3, 2, 1];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Kb1,
    Kb2,
}

impl Track {
    pub fn targets(self) -> &'static [(i16, i16); 4] {
        match self {
            Track::Kb1 => &KB1_TARGETS,
            Track::Kb2 => &KB2_TARGETS,
        }
    }

    pub fn dispatch_order(self) -> &'static [usize; 4] {
        match self {
            Track::Kb1 => &KB1_DISPATCH_ORDER,
            Track::Kb2 => &KB2_DISPATCH_ORDER,
        }
    }
}