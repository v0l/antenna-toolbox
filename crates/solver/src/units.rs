pub const C: f64 = 299_792.458;
pub const ETA: f64 = 376.730_313_668;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Unit {
    #[default]
    Mm,
    Cm,
    In,
}

pub fn format_length(mm: f64, unit: Unit) -> String {
    match unit {
        Unit::Cm => format!("{:.2} cm", mm / 10.0),
        Unit::In => format!("{:.3}\"", mm / 25.4),
        Unit::Mm if mm < 100.0 => format!("{mm:.2} mm"),
        Unit::Mm => format!("{mm:.1} mm"),
    }
}

pub fn wavelength(freq_mhz: f64) -> f64 {
    C / freq_mhz
}
