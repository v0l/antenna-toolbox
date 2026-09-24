pub struct Band {
    pub name: &'static str,
    pub low: f64,
    pub high: f64,
}

const fn band(name: &'static str, low: f64, high: f64) -> Band {
    Band { name, low, high }
}

pub const HF_BANDS: [Band; 10] = [
    band("160 m", 1.8, 2.0),
    band("80 m", 3.5, 4.0),
    band("60 m", 5.3515, 5.3665),
    band("40 m", 7.0, 7.3),
    band("30 m", 10.1, 10.15),
    band("20 m", 14.0, 14.35),
    band("17 m", 18.068, 18.168),
    band("15 m", 21.0, 21.45),
    band("12 m", 24.89, 24.99),
    band("10 m", 28.0, 29.7),
];

impl Band {
    pub fn centre(&self) -> f64 {
        (self.low + self.high) / 2.0
    }

    pub fn probes(&self) -> [f64; 3] {
        [self.low, self.centre(), self.high]
    }
}

pub fn lowest_band(freq: f64) -> Option<usize> {
    if freq > HF_BANDS[HF_BANDS.len() - 1].high {
        return None;
    }
    HF_BANDS.iter().position(|b| b.high >= freq)
}

pub fn bands_from(freq: f64) -> &'static [Band] {
    lowest_band(freq).map(|i| &HF_BANDS[i..]).unwrap_or(&[])
}
