use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clutter {
    WaterSea = 1,
    OpenRural = 2,
    Suburban = 3,
    Urban = 4,
    TreesForest = 5,
    DenseUrban = 6,
}

impl Clutter {
    pub const ALL: [Clutter; 6] = [
        Clutter::WaterSea,
        Clutter::OpenRural,
        Clutter::Suburban,
        Clutter::Urban,
        Clutter::TreesForest,
        Clutter::DenseUrban,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Clutter::WaterSea => "water or sea",
            Clutter::OpenRural => "open or rural",
            Clutter::Suburban => "suburban",
            Clutter::Urban => "urban",
            Clutter::TreesForest => "trees or forest",
            Clutter::DenseUrban => "dense urban",
        }
    }

    pub fn height(self) -> f64 {
        match self {
            Clutter::WaterSea | Clutter::OpenRural | Clutter::Suburban => 10.0,
            Clutter::Urban | Clutter::TreesForest => 15.0,
            Clutter::DenseUrban => 20.0,
        }
    }

    pub fn from_code(c: u8) -> Option<Clutter> {
        Clutter::ALL.iter().copied().find(|x| *x as u8 == c)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Frequency,
    AntennaHeight,
    StreetWidth,
    ClutterHeight,
    Distance,
    Percentage,
    Theta,
}

pub fn inverse_ccdf(q: f64) -> f64 {
    const C: [f64; 3] = [2.515517, 0.802853, 0.010328];
    const D: [f64; 3] = [1.432788, 0.189269, 0.001308];
    let x = if q > 0.5 { 1.0 - q } else { q };
    let t = (-2.0 * x.ln()).sqrt();
    let zeta = ((C[2] * t + C[1]) * t + C[0]) / (((D[2] * t + D[1]) * t + D[0]) * t + 1.0);
    let v = t - zeta;
    if q > 0.5 { -v } else { v }
}

pub fn height_gain_correction(
    f_ghz: f64,
    h: f64,
    street: f64,
    r: f64,
    clutter: Clutter,
) -> Result<f64, Error> {
    if !(0.03..=3.0).contains(&f_ghz) {
        return Err(Error::Frequency);
    }
    if h <= 0.0 {
        return Err(Error::AntennaHeight);
    }
    if street <= 0.0 {
        return Err(Error::StreetWidth);
    }
    if r <= 0.0 {
        return Err(Error::ClutterHeight);
    }
    if h >= r {
        return Ok(0.0);
    }
    let h_dif = r - h;
    let theta = (h_dif / street).atan() * 180.0 / PI;
    let k_h2 = 21.8 + 6.2 * f_ghz.log10();
    Ok(match clutter {
        Clutter::WaterSea | Clutter::OpenRural => -k_h2 * (h / r).log10(),
        _ => {
            let nu = 0.342 * f_ghz.sqrt() * (h_dif * theta).sqrt();
            let j = if nu <= -0.78 {
                0.0
            } else {
                6.9 + 20.0 * (((nu - 0.1).powi(2) + 1.0).sqrt() + nu - 0.1).log10()
            };
            j - 6.03
        }
    })
}

fn terrestrial_helper(f_ghz: f64, d_km: f64, p: f64) -> f64 {
    let sigma_l = 4.0;
    let l_l = -2.0 * (10f64.powf(-5.0 * f_ghz.log10() - 12.5) + 10f64.powf(-16.5)).log10();
    let sigma_s = 6.0;
    let l_s = 32.98 + 23.9 * d_km.log10() + 3.0 * f_ghz.log10();
    let (a, b) = (10f64.powf(-0.2 * l_l), 10f64.powf(-0.2 * l_s));
    let sigma_cb = ((sigma_l * sigma_l * a + sigma_s * sigma_s * b) / (a + b)).sqrt();
    -5.0 * (a + b).log10() - sigma_cb * inverse_ccdf(p / 100.0)
}

pub fn terrestrial(f_ghz: f64, d_km: f64, p: f64) -> Result<f64, Error> {
    if !(0.5..=67.0).contains(&f_ghz) {
        return Err(Error::Frequency);
    }
    if d_km < 0.25 {
        return Err(Error::Distance);
    }
    if p <= 0.0 || p >= 100.0 {
        return Err(Error::Percentage);
    }
    Ok(terrestrial_helper(f_ghz, 2.0, p).min(terrestrial_helper(f_ghz, d_km, p)))
}

pub fn aeronautical(f_ghz: f64, theta_deg: f64, p: f64) -> Result<f64, Error> {
    if !(10.0..=100.0).contains(&f_ghz) {
        return Err(Error::Frequency);
    }
    if !(0.0..=90.0).contains(&theta_deg) {
        return Err(Error::Theta);
    }
    if p <= 0.0 || p >= 100.0 {
        return Err(Error::Percentage);
    }
    let k1 = 93.0 * f_ghz.powf(0.175);
    let part1 = (1.0 - p / 100.0).ln();
    let part2 = 0.05 * (1.0 - theta_deg / 90.0) + PI * theta_deg / 180.0;
    let part3 = 0.5 * (90.0 - theta_deg) / 90.0;
    let part4 = 0.6 * inverse_ccdf(p / 100.0);
    Ok((-k1 * part1 / part2.tan()).powf(part3) - 1.0 - part4)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(name: &str) -> Vec<Vec<f64>> {
        let path = format!("{}/tests/data/p2108/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .skip(1)
            .map(|l| l.split(',').map(|v| v.trim().parse().unwrap()).collect())
            .collect()
    }

    #[test]
    fn height_gain_matches_ntia_test_data() {
        for r in rows("HeightGainTerminalCorrectionModelTestData.csv") {
            let clutter = Clutter::from_code(r[4] as u8).unwrap();
            let got = height_gain_correction(r[0], r[1], r[2], r[3], clutter);
            match (r[5] as i32, got) {
                (0, Ok(v)) => assert!((v - r[6]).abs() < 0.1, "{r:?} -> {v}"),
                (0, Err(e)) => panic!("{r:?} -> {e:?}"),
                (_, got) => assert!(got.is_err(), "{r:?} should fail"),
            }
        }
    }

    #[test]
    fn terrestrial_matches_ntia_test_data() {
        for r in rows("TerrestrialStatisticalModelTestData.csv") {
            match (r[3] as i32, terrestrial(r[0], r[1], r[2])) {
                (0, Ok(v)) => assert!((v - r[4]).abs() < 0.1, "{r:?} -> {v}"),
                (0, Err(e)) => panic!("{r:?} -> {e:?}"),
                (_, got) => assert!(got.is_err(), "{r:?} should fail"),
            }
        }
    }

    #[test]
    fn aeronautical_matches_ntia_test_data() {
        for r in rows("AeronauticalStatisticalModelTestData.csv") {
            match (r[3] as i32, aeronautical(r[0], r[1], r[2])) {
                (0, Ok(v)) => assert!((v - r[4]).abs() < 0.1, "{r:?} -> {v}"),
                (0, Err(e)) => panic!("{r:?} -> {e:?}"),
                (_, got) => assert!(got.is_err(), "{r:?} should fail"),
            }
        }
    }
}
