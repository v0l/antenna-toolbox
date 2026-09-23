use crate::draw::{Anchor, Drawing, GOLD, GREEN, GREY};
use crate::feed_detail::vertical;
use crate::{Build, Ctx, DIPOLE_K, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static DIPOLE: Design = Design {
    id: "dipole",
    name: "Vertical dipole",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2 dBi",
    controls: &[],
    polarisation: "**Vertical, omnidirectional in azimuth.** Same doughnut pattern as the ground \
                   plane and 1 dB better, because it has no radials wasting power. Hang it \
                   vertical for an omni; lay it horizontal and it becomes a figure-eight with \
                   nulls off the ends, which is a different antenna for a different job.",
    compute,
};

fn diagram(tot: f64) -> Drawing {
    let w = 640.0;
    let cx = w / 2.0;
    let sc = 240.0 / tot;
    let half = tot * sc / 2.0;
    let cy = half + 50.0;
    let h = 2.0 * half + 110.0;
    let mut g = Drawing::new(w, h);
    g.line(cx, cy - 6.0, cx, cy - half, GOLD, 3.5);
    g.line(cx, cy + 6.0, cx, cy + half, GOLD, 3.5);
    g.dot(cx, cy - 6.0, 2.8, GOLD);
    g.dot(cx, cy + 6.0, 2.8, GREY);
    g.label(cx + 14.0, cy - 6.0, "pin", GOLD, 11.0, Anchor::Start);
    g.label(cx + 14.0, cy + 10.0, "shield", GREY, 11.0, Anchor::Start);
    g.dim(cx - 50.0, cy - half, cx - 50.0, cy + half, "total", -8.0, 4.0, Anchor::End);
    g.dim(cx + 50.0, cy, cx + 50.0, cy - half, "half", 8.0, 0.0, Anchor::Start);
    g.label(cx, h - 14.0, "equal in all horizontal directions", GREEN, 12.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let tot = c.P("total length", DIPOLE_K * c.lam);
    Output {
        spec: "~2.1 dBi · vertical · 73 Ω".into(),
        rows: vec![
            row("**total length**, tip to tip", c.fmt(tot)),
            row("each half, from the SMA", c.fmt(tot / 2.0)),
            total("total wire needed", c.fmt(tot)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[0.0, tot * 0.02, 0.0], [0.0, tot / 2.0, 0.0]], GOLD, 3.5),
                wire(vec![[0.0, -tot * 0.02, 0.0], [0.0, -tot / 2.0, 0.0]], GOLD, 3.5),
            ],
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(0.0),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![vec![[0.0, -tot / 2.0, 0.0], [0.0, tot / 2.0, 0.0]]],
            [0.0; 3],
        )),
        diagram: diagram(tot),
        feed: vertical(
            "upper half, on the centre pin",
            "lower half, on the shield",
            Some("Choke the coax here or the braid becomes part of the antenna."),
        ),
        notes: "**The simplest thing that works, and the reference every other antenna is \
                measured against.** Two quarter-wave wires, one on the pin, one on the shield, hung \
                vertically. No radials, no ground plane, no plate.\n\n**The feedline is the \
                problem.** A vertical dipole puts the coax straight down through its own field, so \
                the braid picks up current, radiates, and distorts the pattern. This is the design \
                that most needs a choke: ferrite beads or several turns of coax immediately below \
                the feedpoint, and keep the coax running away at right angles for the first quarter \
                wavelength if you can.\n\n73 Ω into 50 Ω coax is 1.46:1 SWR and about 0.15 dB of \
                loss, which is nothing. Do not bother matching it."
            .into(),
        cut: None,
    }
}
