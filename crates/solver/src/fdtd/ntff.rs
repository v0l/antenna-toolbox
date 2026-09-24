use super::ETA0;
use super::engine::{Sim, idx};
use num_complex::Complex64 as C64;
use rayon::prelude::*;
use std::f64::consts::PI;

#[derive(Clone)]
struct Patch {
    pos: [f64; 3],
    area: f64,
    j: [C64; 3],
    m: [C64; 3],
}

pub struct Box {
    pub freq: f64,
    patches: Vec<Patch>,
    samples: Vec<(usize, usize, usize, usize, i8)>,
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

impl Box {
    pub fn new(sim: &Sim, inset: usize, freq: f64) -> Box {
        let n = sim.dims();
        let p = sim.pml + inset;
        let lo = [p; 3];
        let hi = [n[0] - 1 - p, n[1] - 1 - p, n[2] - 1 - p];
        let mut patches = Vec::new();
        let mut samples = Vec::new();
        let g = &sim.grid;
        for axis in 0..3 {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            for (side, plane) in [(-1i8, lo[axis]), (1i8, hi[axis])] {
                for a in lo[u]..hi[u] {
                    for b in lo[v]..hi[v] {
                        let mut pos = [0.0; 3];
                        pos[axis] = g.axis(axis)[plane];
                        pos[u] = 0.5 * (g.axis(u)[a] + g.axis(u)[a + 1]);
                        pos[v] = 0.5 * (g.axis(v)[b] + g.axis(v)[b + 1]);
                        let area = sim.ax[u].d[a] * sim.ax[v].d[b];
                        let mut q = [0usize; 3];
                        q[axis] = plane;
                        q[u] = a;
                        q[v] = b;
                        samples.push((axis, q[0], q[1], q[2], side));
                        patches.push(Patch {
                            pos,
                            area,
                            j: [C64::new(0.0, 0.0); 3],
                            m: [C64::new(0.0, 0.0); 3],
                        });
                    }
                }
            }
        }
        Box { freq, patches, samples }
    }

    pub fn accumulate(&mut self, sim: &Sim, t_e: f64, t_h: f64) {
        let n = sim.dims();
        let w = 2.0 * PI * self.freq;
        let pe = C64::from_polar(sim.dt, -w * t_e);
        let ph = C64::from_polar(sim.dt, -w * t_h);
        let (e, h) = (&sim.e, &sim.h);
        self.patches.par_iter_mut().zip(self.samples.par_iter()).for_each(
            |(patch, &(axis, i, j, k, side))| {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let q = [i, j, k];
                let get = |f: &Vec<f32>, r: [usize; 3]| f[idx(n, r[0], r[1], r[2])] as f64;
                let shift = |a: usize, d: usize| {
                    let mut r = q;
                    r[a] += d;
                    r
                };
                let back = |a: usize| {
                    let mut r = q;
                    r[a] -= 1;
                    r
                };
                let back_up = |a: usize, b: usize| {
                    let mut r = q;
                    r[a] -= 1;
                    r[b] += 1;
                    r
                };
                let mut ef = [0.0; 3];
                ef[u] = 0.5 * (get(&e[u], q) + get(&e[u], shift(v, 1)));
                ef[v] = 0.5 * (get(&e[v], q) + get(&e[v], shift(u, 1)));
                let mut hf = [0.0; 3];
                hf[u] = 0.25
                    * (get(&h[u], q)
                        + get(&h[u], back(axis))
                        + get(&h[u], shift(u, 1))
                        + get(&h[u], back_up(axis, u)));
                hf[v] = 0.25
                    * (get(&h[v], q)
                        + get(&h[v], back(axis))
                        + get(&h[v], shift(v, 1))
                        + get(&h[v], back_up(axis, v)));
                let mut nrm = [0.0; 3];
                nrm[axis] = side as f64;
                let jv = cross(nrm, hf);
                let mv = cross(ef, nrm);
                for c in 0..3 {
                    patch.j[c] += ph * jv[c];
                    patch.m[c] += pe * mv[c];
                }
            },
        );
    }

    pub fn gpu_indices(&self, sim: &Sim) -> Vec<u32> {
        let n = sim.dims();
        let mut out = Vec::with_capacity(self.samples.len() * 13);
        for &(axis, i, j, k, side) in &self.samples {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let q = [i, j, k];
            let at = |moves: &[(usize, isize)]| {
                let mut r = [q[0] as isize, q[1] as isize, q[2] as isize];
                for &(a, d) in moves {
                    r[a] += d;
                }
                idx(n, r[0] as usize, r[1] as usize, r[2] as usize) as u32
            };
            out.push(axis as u32 | if side > 0 { 4 } else { 0 });
            out.extend([at(&[]), at(&[(v, 1)]), at(&[]), at(&[(u, 1)])]);
            out.extend([at(&[]), at(&[(axis, -1)]), at(&[(u, 1)]), at(&[(axis, -1), (u, 1)])]);
            out.extend([at(&[]), at(&[(axis, -1)]), at(&[(v, 1)]), at(&[(axis, -1), (v, 1)])]);
        }
        out
    }

    pub fn load(&mut self, acc: &[f32]) {
        for (p, a) in self.patches.iter_mut().zip(acc.chunks_exact(12)) {
            for c in 0..3 {
                p.j[c] = C64::new(a[2 * c] as f64, a[2 * c + 1] as f64);
                p.m[c] = C64::new(a[6 + 2 * c] as f64, a[6 + 2 * c + 1] as f64);
            }
        }
    }

    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    pub fn far(&self, dir: [f64; 3]) -> [C64; 3] {
        let k = 2.0 * PI * self.freq / super::C0;
        let mut nn = [C64::new(0.0, 0.0); 3];
        let mut ll = [C64::new(0.0, 0.0); 3];
        for p in &self.patches {
            let ph = C64::from_polar(
                p.area,
                k * (dir[0] * p.pos[0] + dir[1] * p.pos[1] + dir[2] * p.pos[2]),
            );
            for c in 0..3 {
                nn[c] += p.j[c] * ph;
                ll[c] += p.m[c] * ph;
            }
        }
        let rn: C64 = (0..3).map(|c| nn[c] * dir[c]).sum();
        let perp = [0, 1, 2].map(|c| nn[c] - rn * dir[c]);
        let rl = [
            dir[1] * ll[2] - dir[2] * ll[1],
            dir[2] * ll[0] - dir[0] * ll[2],
            dir[0] * ll[1] - dir[1] * ll[0],
        ];
        let scale = k / (4.0 * PI);
        [0, 1, 2].map(|c| (rl[c] - perp[c] * ETA0) * scale)
    }
}

pub fn intensity(f: &[C64; 3]) -> f64 {
    f.iter().map(|c| c.norm_sqr()).sum::<f64>() / (2.0 * ETA0)
}
