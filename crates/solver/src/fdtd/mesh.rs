use super::{C0, Model, Stuff};
use sims_field::grid::{Grading, graded, pad, smooth};

pub use sims_field::Grid;

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
        let mut features: Vec<(f64, f64)> = Vec::new();
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
                features.push((v, fine));
            }
        }
        for part in &model.parts {
            let Some(b) = part.bounds() else { continue };
            let size = match part.stuff {
                Stuff::Metal => fine,
                Stuff::Dielectric { eps_r, .. } => {
                    let local = lam / 20.0 / eps_r.sqrt();
                    let thick = b.hi[a] - b.lo[a];
                    if thick < 4.0 * local { (thick / 4.0).max(1e-6) } else { local }
                }
            };
            for v in part.planes(a) {
                fixed.push(v);
                features.push((v, size));
            }
            if part.stuff == Stuff::Metal {
                let wide = b.hi[a] - b.lo[a] > 2.0 * fine;
                for (v, inward) in part.rims(a) {
                    if wide {
                        fixed.push(v + inward * fine / 3.0);
                        fixed.push(v - inward * 2.0 * fine / 3.0);
                    } else {
                        fixed.push(v);
                    }
                    features.push((v, fine));
                }
            }
            features.push((b.lo[a], size));
            features.push((b.hi[a], size));
        }
        for d in &model.dielectrics {
            let (lo, hi) = (d.bounds.lo[a], d.bounds.hi[a]);
            fixed.push(lo);
            fixed.push(hi);
            let local = lam / 20.0 / d.eps_r.sqrt();
            let thick = hi - lo;
            let size = if thick < 4.0 * local { (thick / 4.0).max(1e-6) } else { local };
            features.push((lo, size));
            features.push((hi, size));
            features.push((0.5 * (lo + hi), size));
        }
        if let Some(p) = &model.port {
            for v in [p.a[a], p.b[a]] {
                fixed.push(v);
                features.push((v, fine));
            }
            let len = (p.b[a] - p.a[a]).abs();
            if len > 0.0 {
                features.push((0.5 * (p.a[a] + p.b[a]), (len / 2.0).min(fine * 2.0)));
            }
        }
        let lo = bounds.lo[a] - margin;
        let hi = bounds.hi[a] + margin;
        fixed.push(lo);
        fixed.push(hi);
        fixed.sort_by(f64::total_cmp);
        fixed.dedup_by(|p, q| (*p - *q).abs() < 1e-7);
        let mut slow: Vec<(f64, f64, f64)> =
            model.dielectrics.iter().map(|d| (d.bounds.lo[a], d.bounds.hi[a], d.eps_r)).collect();
        for part in &model.parts {
            if let (Stuff::Dielectric { eps_r, .. }, Some(b)) = (part.stuff, part.bounds()) {
                slow.push((b.lo[a], b.hi[a], eps_r));
            }
        }
        let coarse = move |x: f64| {
            slow.iter()
                .filter(|(lo, hi, _)| x >= *lo && x <= *hi)
                .map(|(_, _, er)| coarse_air / er.sqrt())
                .fold(coarse_air, f64::min)
        };
        let grading = Grading { features: &features, coarse: &coarse, ratio, fill: 0.0 };
        axes.push(pad(smooth(graded(&fixed, &grading), 1.5), pml));
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
            parts: Vec::new(),
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
