use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::vertical;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static JPOLE: Design = Design {
    id: "jpole",
    name: "J-pole",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2.7 dBi",
    controls: &[],
    polarisation: "**Vertical, omnidirectional in azimuth.** An end-fed half wave on top of a \
                   quarter-wave matching stub. The stub is meant to cancel itself; what radiates \
                   is the half wave above it.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let a = c.P("A long leg", 0.72 * lam);
    let b = c.P("B short leg", 0.255 * lam);
    let tap = c.P("D feed tap", 0.0175 * lam);
    let w = c.P("W spacing", 0.012 * lam);
    let solve = WireGeometry::new(
        vec![
            vec![[0.0; 3], [0.0, tap, 0.0], [0.0, a, 0.0]],
            vec![[w, 0.0, 0.0], [w, tap, 0.0], [w, b, 0.0]],
            vec![[0.0; 3], [w, 0.0, 0.0]],
            vec![[0.0, tap, 0.0], [w, tap, 0.0]],
        ],
        [w / 2.0, tap, 0.0],
    );
    Output {
        spec: "~2.7 dBi · vertical · 50 Ω at the tap".into(),
        rows: vec![
            row("**A** long leg, full height", c.fmt(a)),
            row("**B** short leg", c.fmt(b)),
            row("**D** feed tap above the bottom", c.fmt(tap)),
            row("**W** spacing between legs, centre to centre", c.fmt(w)),
            row("radiating half wave, above the short leg", c.fmt(a - b)),
            total("total wire", c.fmt(a + b + w)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[0.0, a, 0.0], [0.0; 3], [w, 0.0, 0.0], [w, b, 0.0]], GOLD, 3.5),
                wire(vec![[0.0, tap, 0.0], [w * 0.3, tap, 0.0]], GOLD, 2.0),
                wire(vec![[w * 0.7, tap, 0.0], [w, tap, 0.0]], GOLD, 2.0),
            ],
            feed: Some([w / 2.0, tap, 0.0]),
            omni: true,
            omni_y: Some(a * 0.7),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            pattern_origin: Some([0.0, (a + b) / 2.0, 0.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(solve),
        diagram: Sketch::new()
            .wire(&[(0.0, a), (0.0, 0.0), (w, 0.0), (w, b)], GOLD, 3.0)
            .wire(&[(0.0, tap), (w, tap)], GOLD, 1.5)
            .dim((-0.04 * a, 0.0), (-0.04 * a, a), "A", (-8.0, 4.0), Anchor::End)
            .dim((w + 0.04 * a, 0.0), (w + 0.04 * a, b), "B", (8.0, 4.0), Anchor::Start)
            .dim((w + 0.1 * a, 0.0), (w + 0.1 * a, tap), "D", (8.0, 4.0), Anchor::Start)
            .caption("equal in all horizontal directions", GREEN)
            .tall(380.0)
            .feed((w / 2.0, tap), true)
            .render(),
        feed: vertical(
            "long leg, on the centre pin",
            "short leg, on the shield",
            Some(
                "Slide the tap up and down the stub to set the resistance, then trim the top of \
                 the long leg for resonance.",
            ),
        ),
        notes: "**The Slim Jim's parent, and the favourite two-metre base antenna.** A half \
                wave does the radiating; below it, two parallel legs a quarter wave long form a \
                shorted stub that steps the half wave's few thousand ohms down to 50 at a tap \
                close to the bottom. No radials, no ground plane.\n\n**The coax is the weak \
                point.** A J-pole is an end-fed antenna with an unbalanced feed, and current \
                leaks onto the outside of the coax. A choke a short way below the tap keeps the \
                pattern clean. The Slim Jim folds the half wave back on itself to reduce this, \
                at the cost of a little gain.\n\nMade from 15 mm copper pipe it is sturdy \
                enough to leave on a mast for years. The legs here are thin wire; fat pipe \
                wants the legs a few percent shorter, which the tune button finds."
            .into(),
        cut: None,
    }
}
