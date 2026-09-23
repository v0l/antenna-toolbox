use crate::draw::{Anchor, BOOM, DIM, Drawing, GOLD, GREY, PALE, Rgba, SILVER};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, Wire, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub struct Element {
    pub len: f64,
    pub at: f64,
    pub label: &'static str,
    pub colour: Rgba,
    pub colour3d: Rgba,
    pub feed: bool,
}

pub fn yagi_scene(els: &[Element]) -> (Scene, WireGeometry) {
    let back = els.iter().map(|e| e.at).fold(0.0, f64::max);
    let mut wires: Vec<Wire> =
        vec![Wire { p: vec![[0.0; 3], [0.0, 0.0, -back]], c: BOOM, w: 5.0, thin: false }];
    for e in els {
        let z = -e.at;
        if e.feed {
            wires.push(wire(
                vec![[-e.len / 2.0, 0.0, z], [-e.len * 0.02, 0.0, z]],
                e.colour3d,
                3.5,
            ));
            wires.push(wire(vec![[e.len * 0.02, 0.0, z], [e.len / 2.0, 0.0, z]], e.colour3d, 3.5));
        } else {
            wires.push(wire(vec![[-e.len / 2.0, 0.0, z], [e.len / 2.0, 0.0, z]], e.colour3d, 3.5));
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
        WireGeometry::new(
            els.iter()
                .map(|e| vec![[-e.len / 2.0, 0.0, -e.at], [e.len / 2.0, 0.0, -e.at]])
                .collect(),
            feed,
        ),
    )
}

pub fn yagi_diagram(els: &[Element], fmt: &dyn Fn(f64) -> String) -> Drawing {
    let w = 640.0;
    let pad = 70.0;
    let span = els.iter().map(|e| e.len).fold(0.0, f64::max);
    let depth = els.iter().map(|e| e.at).fold(0.0, f64::max);
    let sc = (w - 2.0 * pad) / span;
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
        } else {
            g.line(cx - half, yy, cx + half, yy, e.colour, 3.5);
        }
        g.label(cx + half + 8.0, yy + 4.0, e.label, e.colour, 12.0, Anchor::Start);
    }
    for pair in els.windows(2) {
        let (a, b) = (y(&pair[0]), y(&pair[1]));
        let x = cx - span * sc / 2.0 - 26.0;
        g.dashed(x, a, x, b, DIM, 3.0, 3.0);
        g.mono(
            x + 8.0,
            (a + b) / 2.0 + 4.0,
            fmt(pair[1].at - pair[0].at),
            DIM,
            12.0,
            Anchor::Start,
        );
    }
    let feed = els.iter().find(|e| e.feed).expect("driven element");
    g.feed_flag(cx, y(feed), false);
    g.beam_label(cx, h - 14.0, None);
    g
}

pub static YAGI2: Design = Design {
    id: "yagi2",
    name: "2-el Yagi",
    group: Group::Beam,
    build: Build::Wire,
    gain: "6 dBi",
    controls: &[ControlId::Spacing],
    polarisation: "**Linear, parallel to the elements.** Elements horizontal gives horizontal \
                   polarisation. Bandwidth is a few percent either side of the design frequency.",
    compute: compute2,
};

fn compute2(c: &Ctx) -> Output {
    let lam = c.lam;
    let spc = c.ctl(ControlId::Spacing).clamp(0.10, 0.30);
    let drv = c.P("driven", 0.4625 * lam);
    let refl = c.P("reflector", 0.50 * lam);
    let s = c.P("spacing", spc * lam);
    let els = [
        Element { len: drv, label: "driven", colour: GOLD, colour3d: GOLD, feed: true, at: 0.0 },
        Element {
            len: refl,
            label: "reflector",
            colour: PALE,
            colour3d: SILVER,
            feed: false,
            at: s,
        },
    ];
    let (scene, solve) = yagi_scene(&els);
    Output {
        spec: "~6 dBi · 10 dB F/B · ~50 Ω".into(),
        rows: vec![
            row("**driven** dipole, tip to tip", c.fmt(drv)),
            row("each driven half (from the SMA)", c.fmt(drv / 2.0)),
            row("**reflector**, one straight wire", c.fmt(refl)),
            row("**spacing** driven to reflector", c.fmt(s)),
            total("boom length needed", c.fmt(s)),
        ],
        scene,
        solve: Geometry::Wire(solve),
        diagram: yagi_diagram(&els, c.fmt),
        feed: inline("dipole half", "dipole half", None),
        notes: "**The lazy option.** Two dipole halves on the SMA, one longer wire behind them on \
                a non-conductive spacer: plastic, wood, foam, hot glue. Nothing connects to the \
                reflector.\n\nAt 0.18 λ spacing it solves to about 50 Ω with 6 dBi and 10 dB front to \
                back. Wider spacing raises impedance and lowers gain. A Moxon beats this on every count except build time."
            .into(),
        cut: None,
    }
}

pub static YAGI3: Design = Design {
    id: "yagi3",
    name: "3-el Yagi",
    group: Group::Beam,
    build: Build::Wire,
    gain: "8 dBi",
    controls: &[],
    polarisation: "**Linear, parallel to the elements.** Same as the 2-element, and narrower \
                   still: a 3-element wire Yagi is usable over about 2 percent of bandwidth before \
                   the pattern starts to fall apart.",
    compute: compute3,
};

fn compute3(c: &Ctx) -> Output {
    let lam = c.lam;
    let refl = c.P("reflector", 0.482 * lam);
    let drv = c.P("driven", 0.472 * lam);
    let dir = c.P("director", 0.442 * lam);
    let s_r = c.P("refl spacing", 0.20 * lam);
    let s_d = c.P("dir spacing", 0.15 * lam);
    let els = [
        Element { len: dir, label: "director", colour: GREY, colour3d: GREY, feed: false, at: 0.0 },
        Element { len: drv, label: "driven", colour: GOLD, colour3d: GOLD, feed: true, at: s_d },
        Element {
            len: refl,
            label: "reflector",
            colour: PALE,
            colour3d: SILVER,
            feed: false,
            at: s_d + s_r,
        },
    ];
    let (scene, solve) = yagi_scene(&els);
    Output {
        spec: "~7.7 dBi · 14 dB F/B · ~35 Ω".into(),
        rows: vec![
            row("**reflector** length", c.fmt(refl)),
            row("**driven** dipole, tip to tip", c.fmt(drv)),
            row("each driven half (from the SMA)", c.fmt(drv / 2.0)),
            row("**director** length", c.fmt(dir)),
            row("spacing reflector to driven", c.fmt(s_r)),
            row("spacing driven to director", c.fmt(s_d)),
            total("boom length needed", c.fmt(s_r + s_d)),
        ],
        scene,
        solve: Geometry::Wire(solve),
        diagram: yagi_diagram(&els, c.fmt),
        feed: inline("dipole half", "dipole half", None),
        notes: "**Roughly double the power of the 2-element, for one more wire.** The director \
                goes in front, shorter than the driven element, and like the reflector it connects \
                to nothing. Element spacing matters more than element length here, so measure the \
                boom carefully.\n\nImpedance falls to about 35 Ω, near 1.6:1 SWR. Live with it, or \
                use a folded dipole as the driven element, or add a hairpin match. Going past three \
                elements on a wire boom is possible but the tolerances stop forgiving you, \
                especially above 1 GHz."
            .into(),
        cut: None,
    }
}
