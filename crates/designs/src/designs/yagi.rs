use crate::draw::{Anchor, BOOM, DIM, Drawing, GOLD, GREY, PALE, Rgba, SILVER};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Row, Scene, Wire, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;

pub struct Element {
    pub len: f64,
    pub at: f64,
    pub label: String,
    pub colour: Rgba,
    pub colour3d: Rgba,
    pub feed: bool,
    pub fold: Option<f64>,
}

pub fn yagi_scene(els: &[Element]) -> (Scene, WireGeometry) {
    let back = els.iter().map(|e| e.at).fold(0.0, f64::max);
    let mut wires: Vec<Wire> =
        vec![Wire { p: vec![[0.0; 3], [0.0, 0.0, -back]], c: BOOM, w: 5.0, thin: false }];
    let mut lines: Vec<Vec<Vec3>> = Vec::new();
    for e in els {
        let z = -e.at;
        let (l, r) = ([-e.len / 2.0, 0.0, z], [e.len / 2.0, 0.0, z]);
        if e.feed {
            wires.push(wire(vec![l, [-e.len * 0.02, 0.0, z]], e.colour3d, 3.5));
            wires.push(wire(vec![[e.len * 0.02, 0.0, z], r], e.colour3d, 3.5));
        } else {
            wires.push(wire(vec![l, r], e.colour3d, 3.5));
        }
        lines.push(vec![l, r]);
        if let Some(s) = e.fold {
            let (lu, ru) = ([-e.len / 2.0, s, z], [e.len / 2.0, s, z]);
            wires.push(wire(vec![l, lu, ru, r], e.colour3d, 3.5));
            lines.push(vec![l, lu, ru, r]);
        }
    }
    let drv = els.iter().find(|e| e.feed).expect("driven element");
    let feed = [0.0, 0.0, -drv.at];
    (
        Scene {
            wires,
            feed: Some(feed),
            beam_from: Some(0.0),
            pol: "polarisation: linear, along the elements".into(),
            pattern_origin: Some(feed),
            ..Default::default()
        },
        WireGeometry::new(lines, feed),
    )
}

pub fn yagi_diagram(els: &[Element], fmt: &dyn Fn(f64) -> String) -> Drawing {
    let w = 640.0;
    let pad = 70.0;
    let span = els.iter().map(|e| e.len).fold(0.0, f64::max);
    let depth = els.iter().map(|e| e.at).fold(0.0, f64::max);
    let sc = ((w - 2.0 * pad) / span).min(440.0 / depth.max(1e-9));
    let h = depth * sc + 2.0 * pad + 20.0;
    let cx = w / 2.0;
    let y = |e: &Element| pad + e.at * sc;
    let mut g = Drawing::new(w, h);
    g.line(cx, pad, cx, pad + depth * sc, BOOM, 6.0);
    for e in els {
        let yy = y(e);
        let half = e.len * sc / 2.0;
        if e.feed {
            g.line(cx - half, yy, cx - 5.0, yy, e.colour, 3.5);
            g.line(cx + 5.0, yy, cx + half, yy, e.colour, 3.5);
            g.dot(cx - 4.0, yy, 2.6, e.colour);
            g.dot(cx + 4.0, yy, 2.6, e.colour);
            if e.fold.is_some() {
                let up = yy - 7.0;
                g.polyline(
                    &[(cx - half, yy), (cx - half, up), (cx + half, up), (cx + half, yy)],
                    e.colour,
                    2.0,
                );
            }
        } else {
            g.line(cx - half, yy, cx + half, yy, e.colour, 3.5);
        }
        g.label(cx + half + 8.0, yy + 4.0, &e.label, e.colour, 12.0, Anchor::Start);
    }
    let x = cx - span * sc / 2.0 - 26.0;
    for pair in els.windows(2) {
        let (a, b) = (y(&pair[0]), y(&pair[1]));
        g.dashed(x, a, x, b, DIM, 3.0, 3.0);
        if b - a > 13.0 {
            g.mono(
                x + 6.0,
                (a + b) / 2.0 + 4.0,
                fmt(pair[1].at - pair[0].at),
                DIM,
                11.0,
                Anchor::Start,
            );
        }
    }
    let feed = els.iter().find(|e| e.feed).expect("driven element");
    g.feed_flag(cx, y(feed), false);
    g.beam_label(cx, h - 14.0, None);
    g
}

pub static YAGI: Design = Design {
    id: "yagi",
    name: "Yagi",
    group: Group::Beam,
    build: Build::Wire,
    gain: "11.2 dBi",
    controls: &[ControlId::Elements, ControlId::Driven, ControlId::Boom, ControlId::BoomDia],
    polarisation: "**Linear, parallel to the elements.** Elements horizontal gives horizontal \
                   polarisation. The longer the boom, the narrower the band it holds its pattern \
                   over, and the tighter the tolerances on every element.",
    compute,
};

const SPACINGS: [f64; 14] = [
    0.075, 0.180, 0.215, 0.250, 0.280, 0.300, 0.315, 0.330, 0.345, 0.360, 0.375, 0.390, 0.400,
    0.400,
];

const DIRECTOR_K: [[f64; 5]; 7] = [
    [0.001, 0.4711, 0.018, 0.08398, 0.965],
    [0.003, 0.462, 0.01941, 0.08543, 0.9697],
    [0.005, 0.4538, 0.02117, 0.0951, 1.007],
    [0.007, 0.4491, 0.02274, 0.08801, 0.9004],
    [0.01, 0.4421, 0.02396, 0.1027, 1.038],
    [0.015, 0.4358, 0.02558, 0.1149, 1.034],
    [0.02, 0.4268, 0.02614, 0.1112, 1.036],
];

pub fn dl6wu_director(n: usize, ed: f64) -> f64 {
    let ed = ed.clamp(0.001, 0.02);
    let f =
        |k: &[f64; 5]| (k[1] - k[2] * (n as f64).ln()) * (1.0 - k[3] * (-k[4] * n as f64).exp());
    let hi = DIRECTOR_K.iter().position(|k| k[0] >= ed).unwrap_or(DIRECTOR_K.len() - 1).max(1);
    let (lo, hi) = (&DIRECTOR_K[hi - 1], &DIRECTOR_K[hi]);
    let t = (ed - lo[0]) / (hi[0] - lo[0]);
    f(lo) + t * (f(hi) - f(lo))
}

pub fn dl6wu_reflector(ed: f64) -> f64 {
    let ed = ed.clamp(0.001, 0.02);
    (((20.0 - 40.0) / (186.8769 * (2.0 / ed).ln() - 320.0)) + 1.0) / 2.0
}

pub fn dl6wu_driven(dd: f64) -> f64 {
    let dd = dd.clamp(0.001, 0.02);
    (0.4777 - 1.0522 * dd + 0.43363 * dd.powf(-0.014891)) / 2.0
}

pub fn dl6wu_boom_correction(boom_dia: f64, mount: usize) -> f64 {
    let bd = boom_dia.min(0.055);
    let bc = 733.0 * bd * (0.055 - bd) - 504.0 * bd * (0.03 - bd);
    match mount {
        1 => bc * bd,
        2 => bc * bd / 2.0,
        _ => 0.0,
    }
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let n = c.ctl(ControlId::Elements) as usize;
    let folded = c.ctl(ControlId::Driven) == 1.0;
    let mount = c.ctl(ControlId::Boom) as usize;
    let boom_dia = c.ctl(ControlId::BoomDia);
    let ed = c.wire_dia / lam;
    let fold = folded.then(|| c.P("fold spacing", 0.01 * lam));

    let mut spec: Vec<(String, f64, f64, bool)> = Vec::new();
    if n == 2 {
        let refl = c.P("reflector", 0.50 * lam);
        let drv = c.P("driven", 0.4625 * lam);
        let s = c.P("reflector spacing", 0.18 * lam);
        spec.push(("reflector".into(), refl, 0.0, false));
        spec.push(("driven".into(), drv, s, true));
    } else {
        let refl = c.P("reflector", dl6wu_reflector(ed) * lam);
        let base = dl6wu_driven(ed) * if folded { 0.983 } else { 1.0 };
        let drv = c.P("driven", base * lam);
        let mut at = c.P("reflector spacing", 0.2 * lam);
        spec.push(("reflector".into(), refl, 0.0, false));
        spec.push(("driven".into(), drv, at, true));
        for d in 1..=n - 2 {
            let gap = SPACINGS[(d - 1).min(SPACINGS.len() - 1)];
            at += c.P(&format!("D{d} spacing"), gap * lam);
            let len = c.P(&format!("D{d}"), dl6wu_director(d, ed) * lam);
            spec.push((format!("D{d}"), len, at, false));
        }
    }
    let boom = spec.last().map(|s| s.2).unwrap_or(0.0);
    let els: Vec<Element> = spec
        .iter()
        .rev()
        .map(|(label, len, at, feed)| {
            let (colour, colour3d) = match (label.as_str(), feed) {
                (_, true) => (GOLD, GOLD),
                ("reflector", _) => (PALE, SILVER),
                _ => (GREY, GREY),
            };
            Element {
                len: *len,
                at: boom - at,
                label: label.clone(),
                colour,
                colour3d,
                feed: *feed,
                fold: if *feed { fold } else { None },
            }
        })
        .collect();
    let (scene, solve) = yagi_scene(&els);

    let bc = dl6wu_boom_correction(boom_dia / lam, mount) * lam;
    let mut rows: Vec<Row> = Vec::new();
    for (label, len, at, feed) in &spec {
        let cut = if *feed { *len } else { len + bc };
        let name = if *feed {
            format!("**{label}**{}, tip to tip", if folded { " folded dipole" } else { "" })
        } else {
            format!("**{label}**")
        };
        let name =
            if *at > 0.0 { format!("{name}, {} from the reflector", c.fmt(*at)) } else { name };
        rows.push(row(name, c.fmt(cut)));
    }
    if let Some(s) = fold {
        rows.push(row("folded dipole spacing, centre to centre", c.fmt(s)));
    }
    if bc > 0.0 {
        rows.push(row(
            "boom correction already added to every parasitic element",
            format!("+{}", c.fmt(bc)),
        ));
    }
    rows.push(total("boom length, reflector to last director", c.fmt(boom)));

    let spec_line = match (n, folded) {
        (2, false) => "~6 dBi · 10 dB F/B · ~50 Ω".to_string(),
        (_, true) => format!("{n} elements · DL6WU · ~200 Ω folded dipole, 4:1 balun to 50 Ω"),
        _ => format!("{n} elements · DL6WU · 45 to 70 Ω"),
    };
    Output {
        spec: spec_line,
        rows,
        scene,
        solve: Geometry::Wire(solve),
        diagram: yagi_diagram(&els, c.fmt),
        feed: inline(
            if folded { "folded dipole, one end" } else { "dipole half" },
            if folded { "folded dipole, other end" } else { "dipole half" },
            folded.then_some(
                "A folded dipole is about 200 Ω, so it wants a 4:1 balun rather than coax straight \
                 on. The coax loop balun in the matching section does it.",
            ),
        ),
        notes: "**One wire in front of another, then as many more as the boom will take.** The \
                reflector sits behind the driven element and the directors in front, all \
                connected to nothing. From three elements up the lengths and spacings follow \
                DL6WU's long Yagi series, the design most VHF and UHF Yagis are built from: \
                spacings widen along the boom to 0.4 λ, and directors shorten logarithmically, \
                which keeps the gain growing about 2.35 dB for every doubling of boom length \
                instead of saturating. The solved gain tracks DL6WU's own estimate, 9.2 + 3.39 ln(boom \
                in λ) dBd, to within about 0.2 dB.\n\n**Straight or folded driven element.** With \
                these spacings a plain dipole lands between about 45 and 70 Ω, close enough for \
                coax. DL6WU built his with a folded dipole, four times the impedance, into a 4:1 \
                balun; it wants to be about 2 % shorter than the straight one, which the folded \
                option already does. Two elements use a separately tuned reflector and dipole \
                that land on 50 Ω.\n\n**Metal booms change \
                the lengths.** Elements through a metal boom look shorter to the antenna, so the \
                cut lengths grow by DL6WU's boom correction, which depends on the boom's \
                diameter in wavelengths and whether the elements touch it. The model is solved \
                without a boom, so it uses the lengths before that correction. Every element \
                length and spacing can be tweaked below."
            .into(),
        cut: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dl6wu_lengths_match_the_published_program() {
        assert!(
            (dl6wu_director(1, 0.003) - 0.462 * (1.0 - 0.08543 * (-0.9697f64).exp())).abs() < 1e-9
        );
        let r = dl6wu_reflector(0.003);
        assert!((r - 0.4888).abs() < 5e-4, "{r}");
        let d = dl6wu_driven(0.003);
        assert!((d - 0.4737).abs() < 5e-4, "{d}");
        let mid = dl6wu_director(5, 0.002);
        assert!(mid < dl6wu_director(5, 0.001) && mid > dl6wu_director(5, 0.003));
    }
}
