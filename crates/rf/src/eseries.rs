pub const E12: [f64; 12] = [1.0, 1.2, 1.5, 1.8, 2.2, 2.7, 3.3, 3.9, 4.7, 5.6, 6.8, 8.2];

pub const E24: [f64; 24] = [
    1.0, 1.1, 1.2, 1.3, 1.5, 1.6, 1.8, 2.0, 2.2, 2.4, 2.7, 3.0, 3.3, 3.6, 3.9, 4.3, 4.7, 5.1, 5.6,
    6.2, 6.8, 7.5, 8.2, 9.1,
];

pub fn nearest(v: f64, series: &[f64]) -> f64 {
    if v.is_nan() || v <= 0.0 || !v.is_finite() {
        return v;
    }
    let decade = 10f64.powf(v.log10().floor());
    let m = v / decade;
    let mut best = (f64::INFINITY, v);
    for &s in series.iter().chain(std::iter::once(&10.0)) {
        let d = (m / s).ln().abs();
        if d < best.0 {
            best = (d, s * decade);
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_nearest_preferred_value() {
        assert!((nearest(4.5e-12, &E12) - 4.7e-12).abs() < 1e-18);
        assert!((nearest(9.6e-9, &E12) - 10e-9).abs() < 1e-15);
        assert!((nearest(38.8e-9, &E24) - 39e-9).abs() < 1e-15);
    }
}
