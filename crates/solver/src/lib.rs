pub mod analysis;
pub mod cma;
pub mod fdtd;
pub mod geometry;
#[cfg(not(target_arch = "wasm32"))]
pub mod gpu;
#[cfg(target_arch = "wasm32")]
pub mod gpu {
    use crate::linalg::System;
    use crate::surface::mesh::SurfaceTopology;

    pub fn init() -> Option<String> {
        None
    }

    pub fn ready() -> bool {
        false
    }

    pub fn disable() {}

    pub fn fill_surface(_: &SurfaceTopology, _: f64, _: &[usize]) -> Result<System, String> {
        Err("no GPU in the browser build".into())
    }
}
pub mod linalg;
pub mod mom;
pub mod near;
pub mod nec;
pub mod optimise;
pub mod po;
pub mod polarisation;
pub mod rational;
pub mod solve;
pub mod sommerfeld;
pub mod surface;
pub mod symeig;
pub mod units;
pub mod vec;

pub use num_complex::Complex64 as C64;
