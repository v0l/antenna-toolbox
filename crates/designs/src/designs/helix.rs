use crate::draw::{Anchor, DIM, Drawing, GOLD, GREY, METAL, MUTED, Stroke};
use crate::feed_detail::FeedDetail;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Poly, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::{Vec3, ring};
use std::f64::consts::PI;

pub static HELIX: Design = Design {
    id: "helix",
    name: "Axial helix",
    group: Group::Beam,
    build: Build::Wire,
    gain: "13 dBi",
    controls: &[ControlId::Turns],
    polarisation: "**Circular, handed by the winding direction.** Point your right thumb along the \
                   beam: if your fingers curl the way the wire winds, it is right-hand circular. \
                   Wind the other way for left-hand. Two helices only hear each other properly if \
                   they share a hand, and the wrong hand costs you 20 dB or more. Against any \
                   linear antenna you lose a flat 3 dB no matter how it is rotated, which is either \
                   a bargain or a waste depending on what you are pointing at.",
    compute,
};

fn diagram(d: f64, s: f64, n: f64, gp: f64) -> Drawing {
    let w = 640.0;
    let (pad_l, pad_r) = (96.0, 40.0);
    let sc = ((w - pad_l - pad_r) / (n * s)).min(300.0 / gp);
    let cy = 40.0 + gp * sc / 2.0;
    let h = cy + gp * sc / 2.0 + 46.0;
    let r = d * sc / 2.0;
    let x0 = pad_l;
    let step = s * sc;
    let per_turn = 24.0;
    let pts: Vec<(f64, f64)> = (0..=(n * per_turn) as usize)
        .map(|i| {
            let t = i as f64 / per_turn;
            (x0 + t * step, cy - r * (2.0 * PI * t).cos())
        })
        .collect();
    let mut g = Drawing::new(w, h);
    g.line(x0, cy - gp * sc / 2.0, x0, cy + gp * sc / 2.0, METAL, 6.0);
    g.label(x0 - 14.0, cy - gp * sc / 2.0 - 10.0, "ground plane", GREY, 12.0, Anchor::Middle);
    g.polyline(&pts, GOLD, 2.6);
    g.line(x0 - 30.0, cy, x0, cy, MUTED, 4.0);
    g.label(x0 - 36.0, cy + 4.0, "SMA", MUTED, 12.0, Anchor::End);
    g.stroke_path(
        vec![[x0 as f32, (cy + r + 16.0) as f32], [(x0 + step) as f32, (cy + r + 16.0) as f32]],
        false,
        Stroke::dashed(DIM, 1.0, 3.0, 3.0),
    );
    g.mono(x0 + step / 2.0, cy + r + 32.0, "pitch", DIM, 12.0, Anchor::Middle);
    let xe = x0 + step * n + 14.0;
    g.dashed(xe, cy - r, xe, cy + r, DIM, 3.0, 3.0);
    g.mono(xe + 6.0, cy + 4.0, "diameter", DIM, 12.0, Anchor::Start);
    g.beam_label(w / 2.0, h - 12.0, Some("beam fires along the axis, to the right"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let n = c.ctl(ControlId::Turns);
    let circ = c.P("circumference", lam);
    let d = circ / PI;
    let s = c.P("pitch", 0.22 * lam);
    let wire_per_turn = circ.hypot(s);
    let gain = 11.8 + 10.0 * (0.22 * n).log10();
    let bw = 52.0 / (n * 0.22).sqrt();
    let r_h = d / 2.0;
    let gp_r = 0.4 * lam;
    let pts: Vec<Vec3> = (0..=(n as usize * 36))
        .map(|i| {
            let t = i as f64 / 36.0;
            [r_h * (t * 2.0 * PI).cos(), r_h * (t * 2.0 * PI).sin(), t * s]
        })
        .collect();
    let gz = -lam / 60.0;
    Output {
        spec: format!("~{gain:.1} dBi · circular pol · ~140 Ω"),
        rows: vec![
            row("**circumference** of one turn", c.fmt(circ)),
            row("**diameter** of the form", c.fmt(d)),
            row("**pitch**, spacing per turn", c.fmt(s)),
            row("turns", format!("{n}")),
            row("axial length, N × pitch", c.fmt(n * s)),
            row("**ground plane** diameter, min", c.fmt(0.8 * lam)),
            row("beamwidth, estimated", format!("{bw:.0}°")),
            total(format!("total wire, N × {}", c.fmt(wire_per_turn)), c.fmt(n * wire_per_turn)),
        ],
        scene: Scene {
            wires: vec![wire(pts.clone(), GOLD, 2.6)],
            polys: vec![Poly {
                p: ring(48, |a| [gp_r * a.cos(), gp_r * a.sin(), 0.0]),
                fill: None,
                stroke: None,
            }],
            feed: Some([r_h, 0.0, 0.0]),
            beam_from: Some(0.0),
            pol: "polarisation: circular, handed by the winding".into(),
            pattern_origin: Some([0.0, 0.0, n * s / 2.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(
            WireGeometry::new(vec![pts, vec![[r_h, 0.0, gz], [r_h, 0.0, 0.0]]], [r_h, 0.0, gz / 2.0])
                .ground(gz),
        ),
        diagram: diagram(d, s, n, 0.8 * lam),
        feed: FeedDetail::GroundPlane,
        notes: "**The most gain per gram of wire here, and the only circularly polarised one.** \
                Wind the wire around a plastic pipe or a cage of dowels at one turn per pitch \
                length, on a ground plane at least 0.8 λ across. Wind clockwise for right-hand \
                circular, anticlockwise for left. Two helices only talk to each other if they share \
                a hand.\n\n**It does not feed at 50 Ω.** Raw impedance is near 140 Ω. The standard \
                fix costs nothing: flatten the first quarter turn into a wide strip running close \
                and parallel to the ground plane, about 2 mm above it, which tapers the impedance \
                down to roughly 50 Ω. Adjust the height of that strip while watching SWR.\n\nThe \
                gain figure is the Kraus formula, which runs optimistic by 1 to 3 dB, more so below \
                about 6 turns. Treat it as an upper bound. Gain grows with turns and nothing else, \
                so if you want more, make it longer rather than fatter. Bandwidth is enormous, \
                roughly 0.75 to 1.3 times design frequency, which makes this the forgiving choice \
                if you cannot measure."
            .into(),
        cut: None,
    }
}
