use crate::C64;
use crate::cable::{Cable, Run};
use std::f64::consts::PI;

pub const C0: f64 = 299_792_458.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    SeriesL(f64),
    SeriesC(f64),
    ShuntL(f64),
    ShuntC(f64),
    Line(Run),
    OpenStub(Run),
    ShortStub(Run),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Losses {
    pub q_l: f64,
    pub q_c: f64,
}

impl Losses {
    pub const IDEAL: Losses = Losses { q_l: f64::INFINITY, q_c: f64::INFINITY };
}

fn inductor(l: f64, w: f64, q: f64) -> C64 {
    let x = w * l;
    C64::new(if q.is_finite() { x / q } else { 0.0 }, x)
}

fn capacitor(c: f64, w: f64, q: f64) -> C64 {
    let x = -1.0 / (w * c);
    C64::new(if q.is_finite() { x.abs() / q } else { 0.0 }, x)
}

impl Part {
    pub fn abcd(&self, f: f64, loss: Losses) -> [C64; 4] {
        let w = 2.0 * PI * f;
        let one = C64::new(1.0, 0.0);
        let zero = C64::new(0.0, 0.0);
        let series = |z: C64| [one, z, zero, one];
        let shunt = |z: C64| [one, zero, one / z, one];
        match *self {
            Part::SeriesL(l) => series(inductor(l, w, loss.q_l)),
            Part::SeriesC(c) => series(capacitor(c, w, loss.q_c)),
            Part::ShuntL(l) => shunt(inductor(l, w, loss.q_l)),
            Part::ShuntC(c) => shunt(capacitor(c, w, loss.q_c)),
            Part::Line(run) => run.abcd(f),
            Part::OpenStub(run) => shunt(run.toward_source(C64::new(1e15, 0.0), f)),
            Part::ShortStub(run) => shunt(run.toward_source(zero, f)),
        }
    }

    pub fn is_line(&self) -> bool {
        matches!(self, Part::Line(_) | Part::OpenStub(_) | Part::ShortStub(_))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Network {
    pub parts: Vec<Part>,
}

#[derive(Clone, Copy, Debug)]
pub struct Driven {
    pub z_in: C64,
    pub efficiency: f64,
}

impl Network {
    pub fn drive(&self, z_load: C64, f: f64, loss: Losses) -> Driven {
        let (mut v, mut i) = (z_load, C64::new(1.0, 0.0));
        for p in &self.parts {
            let [a, b, c, d] = p.abcd(f, loss);
            (v, i) = (a * v + b * i, c * v + d * i);
        }
        let p_load = z_load.re.max(0.0);
        let p_in = (v * i.conj()).re;
        Driven { z_in: v / i, efficiency: if p_in > 0.0 { (p_load / p_in).min(1.0) } else { 0.0 } }
    }

    pub fn input(&self, z_load: C64, f: f64) -> C64 {
        self.drive(z_load, f, Losses::IDEAL).z_in
    }
}

pub fn line_len(cable: &Cable, f: f64, wavelengths: f64) -> f64 {
    wavelengths * C0 * cable.vf / f
}
