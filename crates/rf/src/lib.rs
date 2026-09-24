pub mod cable;
pub mod eseries;
pub mod network;
pub mod synth;

pub use num_complex::Complex64 as C64;

pub fn gamma(z: C64, z0: f64) -> C64 {
    (z - z0) / (z + z0)
}

pub fn swr(z: C64, z0: f64) -> f64 {
    let g = gamma(z, z0).norm();
    if !g.is_finite() || g >= 0.9999 { 99.0 } else { ((1.0 + g) / (1.0 - g)).min(99.0) }
}
