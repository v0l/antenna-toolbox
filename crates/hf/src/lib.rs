pub mod p372;
pub mod p533;
mod tables;

pub(crate) const PI: f64 = std::f64::consts::PI;
pub(crate) const D2R: f64 = 0.0174532925;
pub(crate) const R2D: f64 = 57.2957795;
pub(crate) const R0: f64 = 6371.009;
pub(crate) const TINYDB: f64 = -307.0;

pub const IONOS_URL: &str =
    "https://raw.githubusercontent.com/ITU-R-Study-Group-3/ITU-R-HF/master/P533/Data";

pub fn ionos_file(month: usize) -> String {
    format!("ionos{:02}.bin", month + 1)
}
