use crate::C64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cable {
    pub name: &'static str,
    pub z0: f64,
    pub vf: f64,
    pub k_sqrt: f64,
    pub k_lin: f64,
}

const FT: f64 = 100.0 / 30.48;

const fn fit(name: &'static str, z0: f64, vf: f64, a: (f64, f64), b: (f64, f64)) -> Cable {
    let (sa, sb) = (sqrt(a.0), sqrt(b.0));
    let det = sa * b.0 - sb * a.0;
    let k_sqrt = (a.1 * b.0 - b.1 * a.0) / det;
    let k_lin = (sa * b.1 - sb * a.1) / det;
    Cable { name, z0, vf, k_sqrt, k_lin }
}

const fn sqrt(x: f64) -> f64 {
    let mut g = if x > 1.0 { x / 2.0 } else { 1.0 };
    let mut i = 0;
    while i < 60 {
        g = 0.5 * (g + x / g);
        i += 1;
    }
    g
}

pub const CABLES: [Cable; 10] = [
    fit("RG-174", 50.0, 0.66, (146.0, 10.2 * FT), (902.0, 26.3 * FT)),
    fit("RG-316", 50.0, 0.69, (146.0, 9.8 * FT), (902.0, 25.1 * FT)),
    fit("RG-58", 50.0, 0.66, (146.0, 5.6 * FT), (902.0, 14.7 * FT)),
    fit("RG-8X", 50.0, 0.78, (146.0, 4.5 * FT), (902.0, 12.4 * FT)),
    fit("RG-213", 50.0, 0.66, (146.0, 2.4 * FT), (902.0, 6.8 * FT)),
    fit("H155", 50.0, 0.79, (100.0, 9.1), (1000.0, 29.6)),
    fit("Aircell 7", 50.0, 0.83, (144.0, 7.22), (1000.0, 20.44)),
    fit("LMR-195", 50.0, 0.80, (150.0, 14.6), (900.0, 36.5)),
    fit("LMR-240", 50.0, 0.84, (150.0, 9.9), (900.0, 24.8)),
    fit("LMR-400", 50.0, 0.85, (150.0, 5.0), (900.0, 12.8)),
];

pub fn cable(name: &str) -> Option<Cable> {
    CABLES.iter().copied().find(|c| c.name == name)
}

impl Cable {
    pub fn ideal(z0: f64, vf: f64) -> Cable {
        Cable { name: "ideal", z0, vf, k_sqrt: 0.0, k_lin: 0.0 }
    }

    pub fn db_per_100m(&self, f_hz: f64) -> f64 {
        let mhz = f_hz / 1e6;
        self.k_sqrt * mhz.sqrt() + self.k_lin * mhz
    }

    pub fn gamma(&self, f_hz: f64) -> C64 {
        let alpha = self.db_per_100m(f_hz) / 100.0 / (20.0 / std::f64::consts::LN_10);
        let beta = 2.0 * std::f64::consts::PI * f_hz / (crate::network::C0 * self.vf);
        C64::new(alpha, beta)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub cable: Cable,
    pub len_m: f64,
}

impl Run {
    pub fn abcd(&self, f_hz: f64) -> [C64; 4] {
        let gl = self.cable.gamma(f_hz) * self.len_m;
        let (ch, sh) = (gl.cosh(), gl.sinh());
        let z0 = self.cable.z0;
        [ch, sh * z0, sh / z0, ch]
    }

    pub fn toward_source(&self, z_load: C64, f_hz: f64) -> C64 {
        let t = (self.cable.gamma(f_hz) * self.len_m).tanh();
        let z0 = self.cable.z0;
        z0 * (z_load + z0 * t) / (z0 + z_load * t)
    }

    pub fn toward_load(&self, z_in: C64, f_hz: f64) -> C64 {
        let t = (self.cable.gamma(f_hz) * self.len_m).tanh();
        let z0 = self.cable.z0;
        z0 * (z_in - z0 * t) / (z0 - z_in * t)
    }

    pub fn loss_db(&self, f_hz: f64) -> f64 {
        self.cable.db_per_100m(f_hz) * self.len_m / 100.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpenFit {
    pub delay_s: f64,
    pub k_sqrt: f64,
    pub k_lin: f64,
}

impl OpenFit {
    pub fn one_way_db(&self, f_hz: f64) -> f64 {
        let mhz = f_hz / 1e6;
        self.k_sqrt * mhz.sqrt() + self.k_lin * mhz
    }

    pub fn toward_load(&self, gamma_in: C64, f_hz: f64) -> C64 {
        let loss = 10f64.powf(self.one_way_db(f_hz) / 10.0);
        let turn = C64::from_polar(1.0, 4.0 * std::f64::consts::PI * f_hz * self.delay_s);
        gamma_in * loss * turn
    }

    pub fn length_m(&self, vf: f64) -> f64 {
        self.delay_s * crate::network::C0 * vf
    }
}

pub fn fit_open(points: &[(f64, C64)], short: bool) -> Option<OpenFit> {
    if points.len() < 3 {
        return None;
    }
    let sign = if short { -1.0 } else { 1.0 };
    let mut phase = Vec::with_capacity(points.len());
    let mut prev = 0.0;
    let mut acc = 0.0;
    for (i, (_, g)) in points.iter().enumerate() {
        let p = (g * sign).arg();
        if i > 0 {
            let mut d = p - prev;
            while d > std::f64::consts::PI {
                d -= 2.0 * std::f64::consts::PI;
            }
            while d < -std::f64::consts::PI {
                d += 2.0 * std::f64::consts::PI;
            }
            acc += d;
        }
        prev = p;
        phase.push(acc);
    }
    let n = points.len() as f64;
    let (mut sf, mut sp, mut sff, mut sfp) = (0.0, 0.0, 0.0, 0.0);
    for ((f, _), p) in points.iter().zip(&phase) {
        sf += f;
        sp += p;
        sff += f * f;
        sfp += f * p;
    }
    let slope = (n * sfp - sf * sp) / (n * sff - sf * sf);
    let delay_s = -slope / (4.0 * std::f64::consts::PI);
    if delay_s.is_nan() || delay_s <= 0.0 {
        return None;
    }
    let (mut a11, mut a12, mut a22, mut b1, mut b2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (f, g) in points {
        let y = -20.0 * g.norm().max(1e-9).log10() / 2.0;
        let (x1, x2) = ((f / 1e6).sqrt(), f / 1e6);
        a11 += x1 * x1;
        a12 += x1 * x2;
        a22 += x2 * x2;
        b1 += x1 * y;
        b2 += x2 * y;
    }
    let det = a11 * a22 - a12 * a12;
    let (mut k_sqrt, mut k_lin) = if det.abs() > 1e-30 {
        ((b1 * a22 - b2 * a12) / det, (a11 * b2 - a12 * b1) / det)
    } else {
        (0.0, 0.0)
    };
    if k_sqrt < 0.0 || k_lin < 0.0 {
        let k = b1 / a11.max(1e-30);
        k_sqrt = k.max(0.0);
        k_lin = 0.0;
    }
    Some(OpenFit { delay_s, k_sqrt, k_lin })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitted_cables_reproduce_their_datasheet_points() {
        let lmr = cable("LMR-400").unwrap();
        assert!((lmr.db_per_100m(150e6) - 5.0).abs() < 1e-9);
        assert!((lmr.db_per_100m(900e6) - 12.8).abs() < 1e-9);
        assert!((lmr.db_per_100m(450e6) - 8.9).abs() < 0.3);
        assert!((lmr.db_per_100m(2500e6) - 22.2).abs() < 1.0);
        let a7 = cable("Aircell 7").unwrap();
        assert!((a7.db_per_100m(432e6) - 12.92).abs() < 0.4);
        let h = cable("H155").unwrap();
        assert!((h.db_per_100m(400e6) - 18.0).abs() < 0.6);
    }

    #[test]
    fn a_line_and_its_inverse_cancel() {
        let run = Run { cable: cable("RG-58").unwrap(), len_m: 3.7 };
        let zl = C64::new(23.0, -41.0);
        for f in [10e6, 162e6, 868e6] {
            let back = run.toward_load(run.toward_source(zl, f), f);
            assert!((back - zl).norm() < 1e-9, "{f} {back}");
        }
    }

    #[test]
    fn an_open_cable_sweep_gives_back_its_length_and_loss() {
        let run = Run { cable: cable("RG-58").unwrap(), len_m: 2.5 };
        let pts: Vec<(f64, C64)> = (0..201)
            .map(|i| {
                let f = 50e6 + i as f64 * 5e6;
                let z = run.toward_source(C64::new(1e12, 0.0), f);
                (f, crate::gamma(z, 50.0))
            })
            .collect();
        let fit = fit_open(&pts, false).unwrap();
        assert!((fit.length_m(0.66) - 2.5).abs() < 1e-3, "{}", fit.length_m(0.66));
        for f in [100e6, 868e6] {
            assert!((fit.one_way_db(f) - run.loss_db(f)).abs() < 0.01);
        }
        let zl = C64::new(30.0, 20.0);
        let g_in = crate::gamma(run.toward_source(zl, 868e6), 50.0);
        let g = fit.toward_load(g_in, 868e6);
        let z = 50.0 * (1.0 + g) / (1.0 - g);
        assert!((z - zl).norm() < 0.5, "{z}");
    }
}
