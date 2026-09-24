pub const ETA0: f64 = 376.730_313_668;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    IcnirpPublic,
    IcnirpWorker,
    FccPublic,
    FccWorker,
}

impl Standard {
    pub const ALL: [Standard; 4] =
        [Standard::IcnirpPublic, Standard::IcnirpWorker, Standard::FccPublic, Standard::FccWorker];

    pub fn label(self) -> &'static str {
        match self {
            Standard::IcnirpPublic => "ICNIRP 2020, public",
            Standard::IcnirpWorker => "ICNIRP 2020, occupational",
            Standard::FccPublic => "FCC, general population",
            Standard::FccWorker => "FCC, occupational",
        }
    }

    pub fn averaging_min(self) -> f64 {
        match self {
            Standard::FccWorker => 6.0,
            _ => 30.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub e: Option<f64>,
    pub h: Option<f64>,
    pub s: Option<f64>,
}

pub fn limits(std: Standard, f_hz: f64) -> Option<Limits> {
    let f = f_hz / 1e6;
    let l = |e: Option<f64>, h: Option<f64>, s: Option<f64>| Some(Limits { e, h, s });
    match std {
        Standard::IcnirpPublic => match f {
            f if !(0.1..=300_000.0).contains(&f) => None,
            f if f <= 30.0 => l(Some(300.0 / f.powf(0.7)), Some(2.2 / f), None),
            f if f <= 400.0 => l(Some(27.7), Some(0.073), Some(2.0)),
            f if f <= 2000.0 => l(Some(1.375 * f.sqrt()), Some(0.0037 * f.sqrt()), Some(f / 200.0)),
            _ => l(None, None, Some(10.0)),
        },
        Standard::IcnirpWorker => match f {
            f if !(0.1..=300_000.0).contains(&f) => None,
            f if f <= 30.0 => l(Some(660.0 / f.powf(0.7)), Some(4.9 / f), None),
            f if f <= 400.0 => l(Some(61.0), Some(0.16), Some(10.0)),
            f if f <= 2000.0 => l(Some(3.0 * f.sqrt()), Some(0.008 * f.sqrt()), Some(f / 40.0)),
            _ => l(None, None, Some(50.0)),
        },
        Standard::FccPublic => match f {
            f if !(0.3..=100_000.0).contains(&f) => None,
            f if f <= 1.34 => l(Some(614.0), Some(1.63), Some(1000.0)),
            f if f <= 30.0 => l(Some(824.0 / f), Some(2.19 / f), Some(1800.0 / (f * f))),
            f if f <= 300.0 => l(Some(27.5), Some(0.073), Some(2.0)),
            f if f <= 1500.0 => l(None, None, Some(f / 150.0)),
            _ => l(None, None, Some(10.0)),
        },
        Standard::FccWorker => match f {
            f if !(0.3..=100_000.0).contains(&f) => None,
            f if f <= 3.0 => l(Some(614.0), Some(1.63), Some(1000.0)),
            f if f <= 30.0 => l(Some(1842.0 / f), Some(4.89 / f), Some(9000.0 / (f * f))),
            f if f <= 300.0 => l(Some(61.4), Some(0.163), Some(10.0)),
            f if f <= 1500.0 => l(None, None, Some(f / 30.0)),
            _ => l(None, None, Some(50.0)),
        },
    }
}

impl Limits {
    pub fn e_or_plane_wave(&self) -> Option<f64> {
        self.e.or(self.s.map(|s| (s * ETA0).sqrt()))
    }

    pub fn h_or_plane_wave(&self) -> Option<f64> {
        self.h.or(self.s.map(|s| (s / ETA0).sqrt()))
    }

    pub fn ratio(&self, e_rms: f64, h_rms: f64) -> f64 {
        let e = self.e_or_plane_wave().map_or(0.0, |l| (e_rms / l).powi(2));
        let h = self.h_or_plane_wave().map_or(0.0, |l| (h_rms / l).powi(2));
        e.max(h)
    }

    pub fn far_distance(&self, eirp_w: f64) -> Option<f64> {
        let s = self.s.or(self.e.map(|e| e * e / ETA0))?;
        Some((eirp_w / (4.0 * std::f64::consts::PI * s)).sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_values() {
        let p = limits(Standard::IcnirpPublic, 162e6).unwrap();
        assert_eq!(p.e, Some(27.7));
        let p = limits(Standard::IcnirpPublic, 868e6).unwrap();
        assert!((p.s.unwrap() - 4.34).abs() < 1e-9);
        assert!((p.e.unwrap() - 40.51).abs() < 0.01);
        let f = limits(Standard::FccPublic, 868e6).unwrap();
        assert!((f.s.unwrap() - 5.7867).abs() < 1e-3);
        let f = limits(Standard::FccPublic, 14e6).unwrap();
        assert!((f.e.unwrap() - 58.857).abs() < 1e-3);
    }

    #[test]
    fn a_plane_wave_at_the_limit_scores_one() {
        let l = limits(Standard::FccPublic, 900e6).unwrap();
        let e = l.e_or_plane_wave().unwrap();
        assert!((l.ratio(e, e / ETA0) - 1.0).abs() < 1e-9);
        let d = l.far_distance(100.0).unwrap();
        let s = 100.0 / (4.0 * std::f64::consts::PI * d * d);
        assert!((s - l.s.unwrap()).abs() < 1e-9);
    }
}
