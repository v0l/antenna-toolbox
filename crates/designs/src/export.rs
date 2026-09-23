use crate::CutFile;
use antenna_solver::vec::Vec3;
use std::fmt::Write;

pub type P2 = [f64; 2];

pub fn sample_curve(f: impl Fn(f64) -> P2, t0: f64, t1: f64, chord: f64) -> Vec<P2> {
    const PROBE: usize = 64;
    let mut len = 0.0;
    let mut prev = f(t0);
    for i in 1..=PROBE {
        let p = f(t0 + (t1 - t0) * i as f64 / PROBE as f64);
        len += (p[0] - prev[0]).hypot(p[1] - prev[1]);
        prev = p;
    }
    let n = PROBE.max((len / chord).ceil() as usize);
    (0..=n).map(|i| f(t0 + (t1 - t0) * i as f64 / n as f64)).collect()
}

pub fn flat(pts: &[P2]) -> Vec<Vec3> {
    pts.iter().map(|p| [p[0], p[1], 0.0]).collect()
}

pub fn meridian_length(r: f64, focal: f64) -> f64 {
    let u = r / (2.0 * focal);
    r / 2.0 * (1.0 + u * u).sqrt() + focal * u.asinh()
}

pub struct Gores {
    pub petal: Vec<Vec3>,
    pub layout: Vec<Vec<Vec3>>,
    pub length: f64,
    pub width: f64,
}

pub fn gores(diameter: f64, focal: f64, segments: usize) -> Gores {
    let steps = 48;
    let r_max = diameter / 2.0;
    let d_phi = 2.0 * std::f64::consts::PI / segments as f64;
    let mut upper = Vec::new();
    let mut lower = Vec::new();
    for i in 0..=steps {
        let r = r_max * i as f64 / steps as f64;
        let s = meridian_length(r, focal);
        let half = r * d_phi / 2.0;
        upper.push([s, half, 0.0]);
        lower.push([s, -half, 0.0]);
    }
    lower.reverse();
    lower.pop();
    let petal: Vec<Vec3> = upper.into_iter().chain(lower).collect();
    let length = meridian_length(r_max, focal);
    let width = r_max * d_phi;
    let gap = width * 0.25;
    let layout = (0..segments)
        .map(|n| {
            let off = n as f64 * (width + gap);
            petal.iter().map(|p| [p[0], p[1] + off, 0.0]).collect()
        })
        .collect();
    Gores { petal, layout, length, width }
}

fn pair(out: &mut String, code: impl std::fmt::Display, value: impl std::fmt::Display) {
    let _ = write!(out, "{code}\n{value}\n");
}

fn num(v: f64) -> String {
    format!("{v:.4}")
}

pub fn cut_to_dxf(cut: &CutFile, name: &str, freq_mhz: f64) -> String {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let mut grow = |x: f64, y: f64| {
        lo = [lo[0].min(x), lo[1].min(y)];
        hi = [hi[0].max(x), hi[1].max(y)];
    };
    for p in cut.loops.iter().flatten() {
        grow(p[0], p[1]);
    }
    for (c, r) in &cut.circles {
        grow(c[0] - r, c[1] - r);
        grow(c[0] + r, c[1] + r);
    }

    let mut o = String::new();
    pair(&mut o, 999, format!("{name}, cut outline at {freq_mhz} MHz, millimetres"));
    pair(&mut o, 999, &cut.note);
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "HEADER");
    pair(&mut o, 9, "$ACADVER");
    pair(&mut o, 1, "AC1009");
    pair(&mut o, 9, "$INSUNITS");
    pair(&mut o, 70, 4);
    pair(&mut o, 9, "$MEASUREMENT");
    pair(&mut o, 70, 1);
    pair(&mut o, 9, "$EXTMIN");
    pair(&mut o, 10, num(lo[0]));
    pair(&mut o, 20, num(lo[1]));
    pair(&mut o, 30, 0);
    pair(&mut o, 9, "$EXTMAX");
    pair(&mut o, 10, num(hi[0]));
    pair(&mut o, 20, num(hi[1]));
    pair(&mut o, 30, 0);
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "TABLES");
    pair(&mut o, 0, "TABLE");
    pair(&mut o, 2, "LAYER");
    pair(&mut o, 70, 1);
    pair(&mut o, 0, "LAYER");
    pair(&mut o, 2, "CUT");
    pair(&mut o, 70, 0);
    pair(&mut o, 62, 7);
    pair(&mut o, 6, "CONTINUOUS");
    pair(&mut o, 0, "ENDTAB");
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "ENTITIES");
    for lp in &cut.loops {
        pair(&mut o, 0, "POLYLINE");
        pair(&mut o, 8, "CUT");
        pair(&mut o, 66, 1);
        pair(&mut o, 70, 1);
        pair(&mut o, 10, 0);
        pair(&mut o, 20, 0);
        pair(&mut o, 30, 0);
        for v in lp {
            pair(&mut o, 0, "VERTEX");
            pair(&mut o, 8, "CUT");
            pair(&mut o, 10, num(v[0]));
            pair(&mut o, 20, num(v[1]));
            pair(&mut o, 30, 0);
        }
        pair(&mut o, 0, "SEQEND");
        pair(&mut o, 8, "CUT");
    }
    for (c, r) in &cut.circles {
        pair(&mut o, 0, "CIRCLE");
        pair(&mut o, 8, "CUT");
        pair(&mut o, 10, num(c[0]));
        pair(&mut o, 20, num(c[1]));
        pair(&mut o, 30, 0);
        pair(&mut o, 40, num(*r));
    }
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "EOF");
    o
}

pub fn dxf_filename(name: &str, freq_mhz: f64) -> String {
    let mut slug = String::new();
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    format!("{slug}-{}mhz.dxf", freq_mhz.round())
}
