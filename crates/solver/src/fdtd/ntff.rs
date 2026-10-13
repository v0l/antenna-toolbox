use super::ETA0;
use num_complex::Complex64 as C64;
use sims_field::fdtd::Far;

pub struct Box {
    pub freq: f64,
    far: Far,
}

impl Box {
    pub fn new(far: Far) -> Box {
        Box { freq: far.freq, far }
    }

    pub fn patch_count(&self) -> usize {
        self.far.pos.len()
    }

    pub fn far(&self, dir: [f64; 3]) -> [C64; 3] {
        self.far.field(dir).map(|(re, im)| C64::new(re, im))
    }
}

pub fn intensity(f: &[C64; 3]) -> f64 {
    f.iter().map(|c| c.norm_sqr()).sum::<f64>() / (2.0 * ETA0)
}
