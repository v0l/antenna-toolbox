use crate::{C64, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    Open,
    Short,
    Load,
}

impl Standard {
    pub const ALL: [Standard; 3] = [Standard::Open, Standard::Short, Standard::Load];

    pub fn label(self) -> &'static str {
        match self {
            Standard::Open => "open",
            Standard::Short => "short",
            Standard::Load => "load",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Calibration {
    pub freqs: Vec<f64>,
    pub open: Option<Vec<C64>>,
    pub short: Option<Vec<C64>>,
    pub load: Option<Vec<C64>>,
}

struct Terms {
    e00: C64,
    e11: C64,
    track: C64,
}

fn terms(open: C64, short: C64, load: C64) -> Terms {
    let e00 = load;
    let a = open - e00;
    let b = short - e00;
    let e11 = (a + b) / (a - b);
    Terms { e00, e11, track: a * (C64::new(1.0, 0.0) - e11) }
}

impl Calibration {
    pub fn store(&mut self, standard: Standard, sweep: &[Point]) {
        let freqs: Vec<f64> = sweep.iter().map(|p| p.freq).collect();
        if freqs != self.freqs {
            *self = Calibration { freqs, ..Default::default() };
        }
        let s11 = Some(sweep.iter().map(|p| p.s11).collect());
        match standard {
            Standard::Open => self.open = s11,
            Standard::Short => self.short = s11,
            Standard::Load => self.load = s11,
        }
    }

    pub fn has(&self, standard: Standard) -> bool {
        match standard {
            Standard::Open => self.open.is_some(),
            Standard::Short => self.short.is_some(),
            Standard::Load => self.load.is_some(),
        }
    }

    pub fn complete(&self) -> bool {
        Standard::ALL.iter().all(|&s| self.has(s))
    }

    pub fn matches(&self, sweep: &[Point]) -> bool {
        self.complete()
            && sweep.len() == self.freqs.len()
            && sweep.iter().zip(&self.freqs).all(|(p, f)| (p.freq - f).abs() < 1.0)
    }

    pub fn apply(&self, sweep: &[Point]) -> Vec<Point> {
        let (Some(o), Some(s), Some(l)) = (&self.open, &self.short, &self.load) else {
            return sweep.to_vec();
        };
        if !self.matches(sweep) {
            return sweep.to_vec();
        }
        sweep
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let t = terms(o[i], s[i], l[i]);
                let d = p.s11 - t.e00;
                Point { s11: d / (t.track + t.e11 * d), ..*p }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(gamma: C64, e00: C64, e11: C64, track: C64) -> C64 {
        e00 + track * gamma / (C64::new(1.0, 0.0) - e11 * gamma)
    }

    #[test]
    fn sol_recovers_a_known_load_through_a_known_error_box() {
        let (e00, e11, track) = (C64::new(0.1, -0.05), C64::new(-0.2, 0.15), C64::new(0.8, 0.3));
        let pt = |g: C64| vec![Point { freq: 1e8, s11: raw(g, e00, e11, track), s21: None }];
        let mut cal = Calibration::default();
        cal.store(Standard::Open, &pt(C64::new(1.0, 0.0)));
        cal.store(Standard::Short, &pt(C64::new(-1.0, 0.0)));
        cal.store(Standard::Load, &pt(C64::new(0.0, 0.0)));
        let dut = C64::new(0.3, -0.4);
        let got = cal.apply(&pt(dut))[0].s11;
        assert!((got - dut).norm() < 1e-12, "{got}");
    }
}
