use super::{C0, Model};

#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub pml: usize,
}

impl Grid {
    pub fn axis(&self, a: usize) -> &[f64] {
        match a {
            0 => &self.x,
            1 => &self.y,
            _ => &self.z,
        }
    }

    pub fn dims(&self) -> [usize; 3] {
        [self.x.len(), self.y.len(), self.z.len()]
    }

    pub fn cells(&self) -> usize {
        (self.x.len() - 1) * (self.y.len() - 1) * (self.z.len() - 1)
    }

    pub fn nearest(&self, a: usize, v: f64) -> usize {
        let ax = self.axis(a);
        ax.iter()
            .enumerate()
            .min_by(|p, q| (p.1 - v).abs().total_cmp(&(q.1 - v).abs()))
            .map(|p| p.0)
            .unwrap_or(0)
    }

    pub fn uniform(lo: [f64; 3], hi: [f64; 3], n: [usize; 3], pml: usize) -> Grid {
        let line = |a: usize| -> Vec<f64> {
            (0..=n[a]).map(|i| lo[a] + (hi[a] - lo[a]) * i as f64 / n[a] as f64).collect()
        };
        Grid { x: line(0), y: line(1), z: line(2), pml }
    }
}

struct Feature {
    at: f64,
    size: f64,
}

fn fill(fixed: &[f64], features: &[Feature], coarse: &dyn Fn(f64) -> f64, ratio: f64) -> Vec<f64> {
    let h = |x: f64| {
        features.iter().map(|f| f.size + (ratio - 1.0) * (x - f.at).abs()).fold(coarse(x), f64::min)
    };
    let mut out = vec![fixed[0]];
    for w in fixed.windows(2) {
        let (a, b) = (w[0], w[1]);
        let steps = 64;
        let mut integral = vec![0.0; steps + 1];
        for s in 0..steps {
            let x0 = a + (b - a) * s as f64 / steps as f64;
            let x1 = a + (b - a) * (s + 1) as f64 / steps as f64;
            integral[s + 1] = integral[s] + (x1 - x0) / h(0.5 * (x0 + x1));
        }
        let n = integral[steps].ceil().max(1.0) as usize;
        for c in 1..n {
            let target = integral[steps] * c as f64 / n as f64;
            let s = integral.partition_point(|v| *v < target).clamp(1, steps);
            let (i0, i1) = (integral[s - 1], integral[s]);
            let t = if i1 > i0 { (target - i0) / (i1 - i0) } else { 0.0 };
            out.push(a + (b - a) * ((s - 1) as f64 + t) / steps as f64);
        }
        out.push(b);
    }
    out
}

fn smooth(mut line: Vec<f64>, limit: f64) -> Vec<f64> {
    for _ in 0..200 {
        let d: Vec<f64> = line.windows(2).map(|w| w[1] - w[0]).collect();
        let bad = (0..d.len()).find(|&i| {
            (i > 0 && d[i] > limit * d[i - 1]) || (i + 1 < d.len() && d[i] > limit * d[i + 1])
        });
        let Some(i) = bad else {
            break;
        };
        let small = [i.checked_sub(1).map(|j| d[j]), d.get(i + 1).copied()]
            .into_iter()
            .flatten()
            .fold(f64::INFINITY, f64::min);
        let parts = (d[i] / (limit * small)).ceil().max(2.0) as usize;
        let (a, b) = (line[i], line[i + 1]);
        let extra: Vec<f64> = (1..parts).map(|k| a + (b - a) * k as f64 / parts as f64).collect();
        line.splice(i + 1..i + 1, extra);
    }
    line
}

pub fn mesh(model: &Model, pml: usize) -> Option<Grid> {
    let bounds = model.bounds()?;
    let f_max = model.f0 * (1.0 + model.span);
    let lam = C0 / f_max;
    let coarse_air = lam / 20.0;
    let margin = C0 / model.f0 / 4.0;
    let fine = if model.fine > 0.0 { model.fine } else { lam / 80.0 };
    let ratio = 1.3;
    let mut axes: Vec<Vec<f64>> = Vec::new();
    for a in 0..3 {
        let mut fixed: Vec<f64> = Vec::new();
        let mut features: Vec<Feature> = Vec::new();
        let pinned: Vec<f64> = model
            .port
            .iter()
            .flat_map(|p| [p.a[a], p.b[a]])
            .chain(model.metal.iter().flat_map(|m| [m.lo[a], m.hi[a]]).filter(|_| false))
            .collect();
        for (mi, m) in model.metal.iter().enumerate() {
            let wide = m.hi[a] - m.lo[a] > 2.0 * fine;
            for (v, inward) in [(m.lo[a], 1.0), (m.hi[a], -1.0)] {
                let shared = pinned.iter().any(|p| (p - v).abs() < 1e-9)
                    || model.metal.iter().enumerate().any(|(oi, o)| {
                        oi != mi
                            && (0..3)
                                .filter(|t| *t != a)
                                .all(|t| o.lo[t] <= m.hi[t] && o.hi[t] >= m.lo[t])
                            && ((o.lo[a] - v).abs() < 1e-9 || (o.hi[a] - v).abs() < 1e-9)
                    });
                if wide && !shared {
                    fixed.push(v + inward * fine / 3.0);
                    fixed.push(v - inward * 2.0 * fine / 3.0);
                } else {
                    fixed.push(v);
                }
                features.push(Feature { at: v, size: fine });
            }
        }
        for d in &model.dielectrics {
            let (lo, hi) = (d.bounds.lo[a], d.bounds.hi[a]);
            fixed.push(lo);
            fixed.push(hi);
            let local = lam / 20.0 / d.eps_r.sqrt();
            let thick = hi - lo;
            let size = if thick < 4.0 * local { (thick / 4.0).max(1e-6) } else { local };
            features.push(Feature { at: lo, size });
            features.push(Feature { at: hi, size });
            features.push(Feature { at: 0.5 * (lo + hi), size });
        }
        if let Some(p) = &model.port {
            for v in [p.a[a], p.b[a]] {
                fixed.push(v);
                features.push(Feature { at: v, size: fine });
            }
            let len = (p.b[a] - p.a[a]).abs();
            if len > 0.0 {
                features.push(Feature {
                    at: 0.5 * (p.a[a] + p.b[a]),
                    size: (len / 2.0).min(fine * 2.0),
                });
            }
        }
        let lo = bounds.lo[a] - margin;
        let hi = bounds.hi[a] + margin;
        fixed.push(lo);
        fixed.push(hi);
        fixed.sort_by(f64::total_cmp);
        fixed.dedup_by(|p, q| (*p - *q).abs() < 1e-7);
        let dielectrics = &model.dielectrics;
        let coarse = move |x: f64| {
            dielectrics
                .iter()
                .filter(|d| x >= d.bounds.lo[a] && x <= d.bounds.hi[a])
                .map(|d| coarse_air / d.eps_r.sqrt())
                .fold(coarse_air, f64::min)
        };
        let mut line = smooth(fill(&fixed, &features, &coarse, ratio), 1.5);
        let first = line[1] - line[0];
        let last = line[line.len() - 1] - line[line.len() - 2];
        let mut pre: Vec<f64> = (1..=pml).rev().map(|i| line[0] - first * i as f64).collect();
        let post: Vec<f64> = (1..=pml).map(|i| line[line.len() - 1] + last * i as f64).collect();
        pre.append(&mut line);
        pre.extend(post);
        axes.push(pre);
    }
    let z = axes.pop()?;
    let y = axes.pop()?;
    let x = axes.pop()?;
    Some(Grid { x, y, z, pml })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fdtd::{Aabb, Dielectric, Port};

    #[test]
    fn a_patch_mesh_keeps_its_lines_and_grows_smoothly() {
        let m = Model {
            metal: vec![
                Aabb::new([-0.04, -0.04, 0.0], [0.04, 0.04, 0.0]),
                Aabb::new([-0.02, -0.015, 0.0016], [0.02, 0.015, 0.0016]),
            ],
            dielectrics: vec![Dielectric {
                bounds: Aabb::new([-0.04, -0.04, 0.0], [0.04, 0.04, 0.0016]),
                eps_r: 4.3,
                tan_d: 0.02,
            }],
            port: Some(Port { a: [0.005, 0.0, 0.0], b: [0.005, 0.0, 0.0016], r: 50.0 }),
            f0: 2.4e9,
            span: 0.2,
            fine: 0.0,
            ground_plane_z: None,
        };
        let g = mesh(&m, 8).unwrap();
        for (a, must) in [(0, [0.005, 0.005]), (2, [0.0, 0.0016])] {
            for v in must {
                assert!(g.axis(a).iter().any(|x| (x - v).abs() < 1e-7), "{a} {v}");
            }
        }
        let fine = C0 / (m.f0 * (1.0 + m.span)) / 80.0;
        for (v, inward) in [(-0.02, 1.0), (0.02, -1.0)] {
            for off in [inward * fine / 3.0, -inward * 2.0 * fine / 3.0] {
                assert!(g.x.iter().any(|x| (x - (v + off)).abs() < 1e-7), "no line at {v} + {off}");
            }
        }
        for a in 0..3 {
            let ax = g.axis(a);
            for w in ax.windows(3) {
                let (d0, d1) = (w[1] - w[0], w[2] - w[1]);
                assert!(d0 > 0.0 && d1 > 0.0);
                assert!(d1 / d0 < 1.9 && d0 / d1 < 1.9, "axis {a}: {d0} then {d1}");
            }
        }
        let zs = &g.z;
        let inside = zs.iter().filter(|z| **z > 1e-9 && **z < 0.0016 - 1e-9).count();
        assert!(inside >= 3, "{inside} lines inside the substrate");
        assert!(g.cells() < 3_000_000, "{}", g.cells());
    }
}
