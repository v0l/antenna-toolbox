use crate::C64;
use crate::cable::{Cable, Run};
use crate::network::{C0, Network, Part};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    L,
    Pi,
    T,
    Stub,
    Quarter,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::L => "L network",
            Family::Pi => "pi network",
            Family::T => "T network",
            Family::Stub => "single stub",
            Family::Quarter => "line and quarter-wave",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub family: Family,
    pub net: Network,
    pub note: String,
}

pub const LINE_Z0: [f64; 7] = [25.0, 37.5, 50.0, 75.0, 93.0, 100.0, 150.0];

fn series(x: f64, w: f64, z0: f64) -> Option<Part> {
    if x.abs() < 1e-9 * z0 {
        None
    } else if x > 0.0 {
        Some(Part::SeriesL(x / w))
    } else {
        Some(Part::SeriesC(-1.0 / (w * x)))
    }
}

fn shunt(b: f64, w: f64, z0: f64) -> Option<Part> {
    if b.abs() < 1e-9 / z0 {
        None
    } else if b > 0.0 {
        Some(Part::ShuntC(b / w))
    } else {
        Some(Part::ShuntL(-1.0 / (w * b)))
    }
}

fn shunt_first(zl: C64, z0: f64) -> Vec<(f64, f64)> {
    let (r, x) = (zl.re, zl.im);
    let den = r * r + x * x;
    let disc = r / z0 * (den - z0 * r);
    if r <= 0.0 || disc < 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for s in [1.0, -1.0] {
        let b = (x + s * disc.sqrt()) / den;
        if b.abs() < 1e-15 {
            if (r - z0).abs() < 1e-9 * z0 {
                out.push((0.0, -x));
            }
            continue;
        }
        out.push((b, 1.0 / b + x * z0 / r - z0 / (b * r)));
        if disc == 0.0 {
            break;
        }
    }
    out
}

fn series_first(zl: C64, z0: f64) -> Vec<(f64, f64)> {
    let (r, x) = (zl.re, zl.im);
    if r <= 0.0 || r > z0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for s in [1.0, -1.0] {
        out.push((s * (r * (z0 - r)).sqrt() - x, s * ((z0 - r) / r).sqrt() / z0));
        if r == z0 {
            break;
        }
    }
    out
}

fn net(parts: impl IntoIterator<Item = Option<Part>>) -> Network {
    Network { parts: parts.into_iter().flatten().collect() }
}

pub fn l_networks(zl: C64, z0: f64, f: f64) -> Vec<Network> {
    let w = 2.0 * PI * f;
    let mut out: Vec<Network> = Vec::new();
    for (b, x) in shunt_first(zl, z0) {
        out.push(net([shunt(b, w, z0), series(x, w, z0)]));
    }
    for (x, b) in series_first(zl, z0) {
        out.push(net([series(x, w, z0), shunt(b, w, z0)]));
    }
    dedupe(out)
}

pub fn min_q(zl: C64, z0: f64) -> f64 {
    let rp = zl.norm_sqr() / zl.re;
    let (lo, hi) = (rp.min(z0), rp.max(z0));
    (hi / lo - 1.0).max(0.0).sqrt()
}

pub fn pi_networks(zl: C64, z0: f64, f: f64, q: f64) -> Vec<Network> {
    let w = 2.0 * PI * f;
    let rp = zl.norm_sqr() / zl.re;
    let rv = rp.max(z0) / (1.0 + q * q);
    if zl.re <= 0.0 || rv >= rp.min(z0) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (b1, x1) in shunt_first(zl, rv) {
        for (x2, b2) in series_first(C64::new(rv, 0.0), z0) {
            out.push(net([shunt(b1, w, z0), series(x1 + x2, w, z0), shunt(b2, w, z0)]));
            out.retain(|n| n.parts.len() == 3);
        }
    }
    dedupe(out)
}

pub fn t_networks(zl: C64, z0: f64, f: f64, q: f64) -> Vec<Network> {
    let w = 2.0 * PI * f;
    let rv = zl.re.min(z0) * (1.0 + q * q);
    if zl.re <= 0.0 || rv <= zl.re.max(z0) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (x1, b1) in series_first(zl, rv) {
        for (b2, x2) in shunt_first(C64::new(rv, 0.0), z0) {
            out.push(net([series(x1, w, z0), shunt(b1 + b2, w, z0), series(x2, w, z0)]));
            out.retain(|n| n.parts.len() == 3);
        }
    }
    dedupe(out)
}

fn wrap_half(v: f64) -> f64 {
    v.rem_euclid(0.5)
}

pub fn stubs(zl: C64, z0: f64, f: f64, vf: f64) -> Vec<(Network, f64, f64)> {
    let (r, x) = (zl.re, zl.im);
    if r <= 0.0 {
        return Vec::new();
    }
    let ts: Vec<f64> = if (r - z0).abs() < 1e-9 * z0 {
        vec![-x / (2.0 * z0)]
    } else {
        let s = (r * ((z0 - r).powi(2) + x * x) / z0).sqrt();
        vec![(x + s) / (r - z0), (x - s) / (r - z0)]
    };
    let lam = C0 * vf / f;
    let cable = Cable::ideal(z0, vf);
    let mut out = Vec::new();
    for t in ts {
        let d = wrap_half(t.atan() / (2.0 * PI));
        let b = (r * r * t - (z0 - x * t) * (x + z0 * t)) / (z0 * (r * r + (x + z0 * t).powi(2)));
        let open = wrap_half((-b * z0).atan() / (2.0 * PI));
        let short = wrap_half((1.0 / (b * z0)).atan() / (2.0 * PI));
        let line = (d > 1e-9).then_some(Part::Line(Run { cable, len_m: d * lam }));
        out.push((net([line, Some(Part::OpenStub(Run { cable, len_m: open * lam }))]), d, open));
        out.push((net([line, Some(Part::ShortStub(Run { cable, len_m: short * lam }))]), d, short));
    }
    out
}

pub fn quarter_waves(zl: C64, z0: f64, f: f64, vf: f64) -> Vec<(Network, f64, f64)> {
    let g = crate::gamma(zl, z0);
    if zl.re <= 0.0 || g.norm() >= 0.999 {
        return Vec::new();
    }
    let s = (1.0 + g.norm()) / (1.0 - g.norm());
    let lam = C0 * vf / f;
    let theta = g.arg();
    let mut out = Vec::new();
    for (shift, rr) in [(0.0, z0 * s), (PI, z0 / s)] {
        let d = wrap_half((theta + shift) / (4.0 * PI));
        let ideal = (rr * z0).sqrt();
        let zt = LINE_Z0
            .iter()
            .copied()
            .min_by(|a, b| (a / ideal).ln().abs().total_cmp(&(b / ideal).ln().abs()))
            .unwrap();
        let line =
            (d > 1e-9).then_some(Part::Line(Run { cable: Cable::ideal(z0, vf), len_m: d * lam }));
        if (zt - z0).abs() < 1e-9 {
            continue;
        }
        let qw = Part::Line(Run { cable: Cable::ideal(zt, vf), len_m: 0.25 * lam });
        out.push((net([line, Some(qw)]), d, ideal));
    }
    out
}

fn dedupe(v: Vec<Network>) -> Vec<Network> {
    let mut out: Vec<Network> = Vec::new();
    for n in v {
        if !out.iter().any(|o| same(o, &n)) {
            out.push(n);
        }
    }
    out
}

fn same(a: &Network, b: &Network) -> bool {
    a.parts.len() == b.parts.len()
        && a.parts.iter().zip(&b.parts).all(|(p, q)| match (p, q) {
            (Part::SeriesL(x), Part::SeriesL(y))
            | (Part::SeriesC(x), Part::SeriesC(y))
            | (Part::ShuntL(x), Part::ShuntL(y))
            | (Part::ShuntC(x), Part::ShuntC(y)) => ((x - y) / x).abs() < 1e-6,
            _ => p == q,
        })
}

pub fn describe(n: &Network) -> String {
    n.parts
        .iter()
        .map(|p| match p {
            Part::SeriesL(_) => "series L",
            Part::SeriesC(_) => "series C",
            Part::ShuntL(_) => "shunt L",
            Part::ShuntC(_) => "shunt C",
            Part::Line(_) => "line",
            Part::OpenStub(_) => "open stub",
            Part::ShortStub(_) => "shorted stub",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn all(zl: C64, z0: f64, f: f64, q: f64, vf: f64) -> Vec<Solution> {
    let mut out = Vec::new();
    for n in l_networks(zl, z0, f) {
        out.push(Solution { family: Family::L, note: describe(&n), net: n });
    }
    for n in pi_networks(zl, z0, f, q) {
        out.push(Solution { family: Family::Pi, note: describe(&n), net: n });
    }
    for n in t_networks(zl, z0, f, q) {
        out.push(Solution { family: Family::T, note: describe(&n), net: n });
    }
    for (n, d, l) in stubs(zl, z0, f, vf) {
        let note = format!("{} at {d:.3} λ, stub {l:.3} λ", describe(&n));
        out.push(Solution { family: Family::Stub, note, net: n });
    }
    for (n, d, ideal) in quarter_waves(zl, z0, f, vf) {
        let note = format!("{d:.3} λ of line, then λ/4 of {ideal:.0} Ω");
        out.push(Solution { family: Family::Quarter, note, net: n });
    }
    out
}

pub fn swr_band(
    net: &Network,
    sweep: &[(f64, C64)],
    f0: f64,
    z0: f64,
    limit: f64,
) -> Option<(f64, f64)> {
    let swr: Vec<(f64, f64)> =
        sweep.iter().map(|(f, z)| (*f, crate::swr(net.input(*z, *f), z0))).collect();
    let centre =
        swr.iter().enumerate().min_by(|a, b| (a.1.0 - f0).abs().total_cmp(&(b.1.0 - f0).abs()))?.0;
    if swr[centre].1 > limit {
        return None;
    }
    let (mut lo, mut hi) = (centre, centre);
    while lo > 0 && swr[lo - 1].1 <= limit {
        lo -= 1;
    }
    while hi + 1 < swr.len() && swr[hi + 1].1 <= limit {
        hi += 1;
    }
    Some((swr[lo].0, swr[hi].0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::Losses;

    fn value(n: &Network) -> Vec<(char, f64)> {
        n.parts
            .iter()
            .map(|p| match *p {
                Part::SeriesL(v) => ('L', v),
                Part::SeriesC(v) => ('C', v),
                Part::ShuntL(v) => ('l', v),
                Part::ShuntC(v) => ('c', v),
                _ => ('-', 0.0),
            })
            .collect()
    }

    #[test]
    fn pozar_example_5_1_l_section() {
        let f = 500e6;
        let nets = l_networks(C64::new(200.0, -100.0), 100.0, f);
        assert_eq!(nets.len(), 2);
        let a = value(&nets[0]);
        assert_eq!(a[0].0, 'c');
        assert!((a[0].1 * 1e12 - 0.92).abs() < 0.01, "{a:?}");
        assert_eq!(a[1].0, 'L');
        assert!((a[1].1 * 1e9 - 38.8).abs() < 0.2, "{a:?}");
        let b = value(&nets[1]);
        assert_eq!(b[0].0, 'l');
        assert!((b[0].1 * 1e9 - 46.1).abs() < 0.1, "{b:?}");
        assert_eq!(b[1].0, 'C');
        assert!((b[1].1 * 1e12 - 2.61).abs() < 0.02, "{b:?}");
    }

    #[test]
    fn pozar_example_5_2_single_stub() {
        let s = stubs(C64::new(60.0, -80.0), 50.0, 2e9, 1.0);
        let got: Vec<(f64, f64)> = s.iter().skip(1).step_by(2).map(|(_, d, l)| (*d, *l)).collect();
        let want = [(0.110, 0.095), (0.260, 0.405)];
        for w in want {
            assert!(
                got.iter().any(|g| (g.0 - w.0).abs() < 0.002 && (g.1 - w.1).abs() < 0.002),
                "{got:?}"
            );
        }
    }

    #[test]
    fn every_solution_matches_awkward_loads() {
        let f = 162e6;
        for zl in [
            C64::new(12.0, 35.0),
            C64::new(300.0, -150.0),
            C64::new(50.0, 80.0),
            C64::new(3.5, -220.0),
            C64::new(1800.0, 400.0),
        ] {
            let sols = all(zl, 50.0, f, 5.0, 0.66);
            assert!(sols.iter().any(|s| s.family == Family::L));
            for s in &sols {
                let z = s.net.input(zl, f);
                if s.family == Family::Quarter {
                    continue;
                }
                assert!(crate::swr(z, 50.0) < 1.001, "{zl} {:?} {} -> {z}", s.family, s.note);
            }
            for s in sols.iter().filter(|s| s.family == Family::Pi || s.family == Family::T) {
                assert_eq!(s.net.parts.len(), 3, "{}", s.note);
            }
        }
    }

    #[test]
    fn quarter_wave_hits_the_real_point_it_names() {
        let zl = C64::new(30.0, 40.0);
        for (n, _, ideal) in quarter_waves(zl, 50.0, 100e6, 0.66) {
            let mut n = n;
            if let Some(Part::Line(run)) = n.parts.last_mut() {
                run.cable.z0 = ideal;
            }
            assert!(crate::swr(n.input(zl, 100e6), 50.0) < 1.001);
        }
    }

    #[test]
    fn a_lossy_coil_shows_up_as_lost_power() {
        let zl = C64::new(2.0, -300.0);
        let n = &l_networks(zl, 50.0, 14e6)[0];
        let d = n.drive(zl, 14e6, Losses { q_l: 200.0, q_c: 2000.0 });
        assert!(d.efficiency < 0.6 && d.efficiency > 0.2, "{}", d.efficiency);
        let ideal = n.drive(zl, 14e6, Losses::IDEAL);
        assert!((ideal.efficiency - 1.0).abs() < 1e-9);
    }
}
