use crate::draw::{Anchor, BOOM, DIM, Drawing, GOLD, GREY, INK, Stroke, hex, SILVER};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, Wire, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::diamond;
use std::f64::consts::SQRT_2;

pub static QUAD: Design = Design {
    id: "quad",
    name: "2-el quad",
    group: Group::Beam,
    build: Build::Wire,
    gain: "7 dBi",
    controls: &[ControlId::Spacing],
    polarisation: "**Linear, set by where you feed it.** Fed at the bottom or top corner as drawn, \
                   the polarisation is horizontal. Move the feed to a left or right corner and it \
                   becomes vertical, with no change to any dimension. Loops are a little \
                   wider-band than a Yagi.",
    compute,
};

fn diagram(side_d: f64, side_r: f64, spacing: f64) -> Drawing {
    let w = 640.0;
    let r_d = side_d * SQRT_2 / 2.0;
    let r_r = side_r * SQRT_2 / 2.0;
    let sc = w * 0.42 / (2.0 * r_r);
    let ox = spacing * sc * 0.55;
    let oy = -spacing * sc * 0.40;
    let cx = w * 0.40;
    let cy = 60.0 + r_r * sc;
    let h = cy + r_r * sc + 70.0;
    let (dx, dy) = (cx, cy);
    let (rx, ry) = (cx + ox, cy + oy);
    let rd = r_d * sc;
    let rr = r_r * sc;
    let mut g = Drawing::new(w, h);
    let loop_ = |g: &mut Drawing, x: f64, y: f64, r: f64, stroke: Stroke| {
        g.polygon(&[(x, y - r), (x + r, y), (x, y + r), (x - r, y)], None, Some(stroke));
    };
    loop_(&mut g, rx, ry, rr, Stroke::dashed(hex(0x6b757b), 3.5, 7.0, 5.0));
    for ((ax, ay), (bx, by)) in [
        ((dx, dy - rd), (rx, ry - rr)),
        ((dx + rd, dy), (rx + rr, ry)),
        ((dx, dy + rd), (rx, ry + rr)),
        ((dx - rd, dy), (rx - rr, ry)),
    ] {
        g.line(ax, ay, bx, by, BOOM, 1.5);
    }
    loop_(&mut g, dx, dy, rd, Stroke::new(GOLD, 3.5));
    g.rect(dx - 6.0, dy + rd - 6.0, 12.0, 12.0, Some(INK), None);
    g.dot(dx - 4.0, dy + rd, 2.6, GOLD);
    g.dot(dx + 4.0, dy + rd, 2.6, GOLD);
    g.label(dx - rd - 10.0, dy + 4.0, "driven loop", GOLD, 12.0, Anchor::End);
    g.label(rx + 10.0, ry - rr - 8.0, "reflector loop", GREY, 12.0, Anchor::Start);
    g.dashed(dx + rd, dy, rx + rr, ry, DIM, 3.0, 3.0);
    g.mono((dx + rd + rx + rr) / 2.0 + 8.0, (dy + ry) / 2.0 + 22.0, "spacing", DIM, 12.0, Anchor::Start);
    g.feed_flag(dx, dy + rd, true);
    g.beam_label(w / 2.0, h - 14.0, Some("beam fires toward the viewer, away from the reflector"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let spc = c.ctl(ControlId::Spacing).clamp(0.10, 0.25);
    let p_d = c.P("driven perimeter", 1.02 * lam);
    let p_r = c.P("refl perimeter", 1.06 * lam);
    let s = c.P("spacing", spc * lam);
    let r_d = p_d / 4.0 * SQRT_2 / 2.0;
    let r_r = p_r / 4.0 * SQRT_2 / 2.0;
    Output {
        spec: "~7 dBi · 15 dB F/B · 50-75 Ω".into(),
        rows: vec![
            row("**driven** loop perimeter", c.fmt(p_d)),
            row("driven loop, each of 4 sides", c.fmt(p_d / 4.0)),
            row("**reflector** loop perimeter", c.fmt(p_r)),
            row("reflector loop, each of 4 sides", c.fmt(p_r / 4.0)),
            row("**spacing** between loops", c.fmt(s)),
            total("total wire needed", c.fmt(p_d + p_r)),
        ],
        scene: Scene {
            wires: vec![
                wire(
                    vec![
                        [-r_d * 0.03, -r_d, 0.0],
                        [-r_d, 0.0, 0.0],
                        [0.0, r_d, 0.0],
                        [r_d, 0.0, 0.0],
                        [r_d * 0.03, -r_d, 0.0],
                    ],
                    GOLD,
                    3.5,
                ),
                wire(diamond(r_r, -s, 0.0), SILVER, 3.5),
                Wire { p: vec![[0.0; 3], [0.0, 0.0, -s]], c: BOOM, w: 2.0, thin: true },
            ],
            feed: Some([0.0, -r_d, 0.0]),
            beam_from: Some(0.0),
            pol: "polarisation: linear, set by the feed corner".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![diamond(r_d, 0.0, 0.0), diamond(r_r, -s, 0.0)],
            [0.0, -r_d, 0.0],
        )),
        diagram: diagram(p_d / 4.0, p_r / 4.0, s),
        feed: inline(
            "loop end, left side",
            "loop end, right side",
            Some("The driven loop is cut at the feed. The reflector loop stays closed."),
        ),
        notes: "**A full wavelength of wire per loop, bent into a square.** More gain than a \
                2-element Yagi and a much better impedance, at the cost of needing a frame to hold \
                the shape. Two crossed sticks per loop is the classic answer. Diamond orientation \
                as drawn works as well as square, and is easier to support from one mast.\n\n\
                **Polarisation follows the feedpoint.** Fed at the bottom corner as drawn, the \
                polarisation is horizontal. Move the feed to a side corner and it turns vertical. \
                The reflector loop is closed, continuous, and connected to nothing.\n\nImpedance \
                runs 50 to 75 Ω across the usable spacing range, so coax connects directly. A quad \
                is quiet on receive and forgiving about nearby objects, which is why it survives at \
                HF where a Yagi would be unaffordable."
            .into(),
        cut: None,
    }
}
