use crate::solve::SolveResult;
use crate::vec::{Vec3, cross, dot, normalise, scale, sub};

#[derive(Clone, Debug)]
pub struct Cut {
    pub degrees: Vec<f64>,
    pub gain_dbi: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Metrics {
    pub peak_dir: Vec3,
    pub peak_dbi: f64,
    pub peak_elevation: f64,
    pub azimuth: Cut,
    pub elevation: Cut,
    pub front_to_back: f64,
    pub beamwidth_azimuth: Option<f64>,
    pub beamwidth_elevation: Option<f64>,
}

fn frame(up: Vec3, toward: Vec3) -> (Vec3, Vec3, Vec3) {
    let u = normalise(up);
    let mut f = sub(toward, scale(u, dot(toward, u)));
    if dot(f, f) < 1e-9 {
        let seed = if u[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        f = sub(seed, scale(u, dot(seed, u)));
    }
    let f = normalise(f);
    (u, f, cross(u, f))
}

fn beamwidth(cut: &Cut, centre: usize) -> Option<f64> {
    let n = cut.gain_dbi.len();
    let level = cut.gain_dbi[centre] - 3.0;
    let step = |i: usize, dir: isize| ((i as isize + dir).rem_euclid(n as isize)) as usize;
    let edge = |dir: isize| -> Option<f64> {
        let mut i = centre;
        for k in 0..n / 2 {
            let j = step(i, dir);
            if cut.gain_dbi[j] < level {
                let (a, b) = (cut.gain_dbi[i], cut.gain_dbi[j]);
                return Some(k as f64 + (a - level) / (a - b));
            }
            i = j;
        }
        None
    };
    let span = cut.degrees[1] - cut.degrees[0];
    Some((edge(1)? + edge(-1)?) * span)
}

pub fn analyse(r: &SolveResult, up: Vec3) -> Option<Metrics> {
    let pattern = r.pattern.as_ref()?;
    let gain = |d: Vec3| r.gain_dbi(d).unwrap_or(f64::NEG_INFINITY);
    let mut best = (f64::NEG_INFINITY, [0.0, 0.0, 1.0]);
    let u = normalise(up);
    let (_, f0, s0) = frame(u, [1.0, 0.0, 0.0]);
    for i in 0..=90 {
        let el = (i as f64 * 2.0 - 90.0).to_radians();
        for j in 0..180 {
            let az = (j as f64 * 2.0).to_radians();
            let d = [0, 1, 2]
                .map(|k| el.cos() * (az.cos() * f0[k] + az.sin() * s0[k]) + el.sin() * u[k]);
            let v = pattern(d);
            if v > best.0 {
                best = (v, d);
            }
        }
    }
    let peak_dir = best.1;
    let (u, f, s) = frame(u, peak_dir);
    let peak_elevation = dot(peak_dir, u).clamp(-1.0, 1.0).asin().to_degrees();
    let el0 = peak_elevation.to_radians();

    let degrees: Vec<f64> = (0..360).map(|i| i as f64).collect();
    let azimuth = Cut {
        gain_dbi: degrees
            .iter()
            .map(|a| {
                let a = a.to_radians();
                gain(
                    [0, 1, 2]
                        .map(|k| el0.cos() * (a.cos() * f[k] + a.sin() * s[k]) + el0.sin() * u[k]),
                )
            })
            .collect(),
        degrees: degrees.clone(),
    };
    let elevation = Cut {
        gain_dbi: degrees
            .iter()
            .map(|e| {
                let e = e.to_radians();
                gain([0, 1, 2].map(|k| e.cos() * f[k] + e.sin() * u[k]))
            })
            .collect(),
        degrees,
    };
    let el_index = (peak_elevation.round() as i64).rem_euclid(360) as usize;
    let peak_dbi = gain(peak_dir);
    Some(Metrics {
        peak_dir,
        peak_dbi,
        peak_elevation,
        front_to_back: peak_dbi - gain(sub(scale(u, 2.0 * dot(peak_dir, u)), peak_dir)),
        beamwidth_azimuth: beamwidth(&azimuth, 0),
        beamwidth_elevation: beamwidth(&elevation, el_index),
        azimuth,
        elevation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::WireGeometry;
    use crate::solve::{model_for, solve_at};

    #[test]
    fn a_dipole_has_a_78_degree_beam_and_an_omni_azimuth() {
        let geo = WireGeometry::new(vec![vec![[0.0, 0.0, -236.0], [0.0, 0.0, 236.0]]], [0.0; 3]);
        let r = solve_at(&model_for(&geo, 1000.0, 1.0, 900), 1000.0, true);
        let m = analyse(&r, [0.0, 0.0, 1.0]).unwrap();
        assert!(m.peak_elevation.abs() < 2.0, "{}", m.peak_elevation);
        assert!(m.beamwidth_azimuth.is_none());
        let bw = m.beamwidth_elevation.unwrap();
        assert!((bw - 78.0).abs() < 2.0, "{bw}");
        assert!(m.front_to_back.abs() < 0.1);
    }
}
