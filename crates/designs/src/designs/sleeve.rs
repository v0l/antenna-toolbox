use crate::draw::{Anchor, GOLD, GREEN, SILVER};
use crate::feed_detail::vertical;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;
use std::f64::consts::PI;

pub static SLEEVE: Design = Design {
    id: "sleeve",
    name: "Sleeve dipole",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2.2 dBi",
    controls: &[],
    polarisation: "**Vertical, omnidirectional in azimuth.** A vertical dipole whose lower half is \
                   a tube over the coax, so the feedline leaves from the bottom through the middle \
                   of the antenna instead of cutting across it.",
    compute,
};

const CAGE: usize = 6;

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let top = c.P("radiator", 0.233 * lam);
    let sleeve = c.P("sleeve", 0.225 * lam);
    let r = c.P("sleeve radius", 0.01 * lam);
    let g = lam / 200.0;
    let at = |i: usize, y: f64| -> Vec3 {
        let a = i as f64 * 2.0 * PI / CAGE as f64;
        [r * a.cos(), y, r * a.sin()]
    };
    let mut lines: Vec<Vec<Vec3>> =
        vec![vec![[0.0, -g, 0.0], [0.0, g, 0.0]], vec![[0.0, g, 0.0], [0.0, g + top, 0.0]]];
    let mut wires = vec![wire(vec![[0.0, g, 0.0], [0.0, g + top, 0.0]], GOLD, 3.5)];
    for i in 0..CAGE {
        lines.push(vec![[0.0, -g, 0.0], at(i, -g)]);
        lines.push(vec![at(i, -g), at(i, -g - sleeve)]);
        wires.push(wire(vec![[0.0, -g, 0.0], at(i, -g), at(i, -g - sleeve)], SILVER, 2.0));
    }
    Output {
        spec: "~2 dBi · vertical · 70 Ω, coax up the middle".into(),
        rows: vec![
            row("**radiator**, the coax centre conductor above the sleeve", c.fmt(top)),
            row("**sleeve**, tube or braid over the coax", c.fmt(sleeve)),
            row("sleeve diameter", c.fmt(2.0 * r)),
            total("overall height", c.fmt(top + sleeve + 2.0 * g)),
        ],
        scene: Scene {
            wires,
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(0.0),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(lines, [0.0; 3])),
        diagram: Sketch::new()
            .wire(&[(0.0, g), (0.0, g + top)], GOLD, 3.0)
            .wire(&[(-r, -g), (r, -g)], SILVER, 2.0)
            .wire(&[(-r, -g), (-r, -g - sleeve)], SILVER, 2.5)
            .wire(&[(r, -g), (r, -g - sleeve)], SILVER, 2.5)
            .wire(&[(0.0, -g), (0.0, -g - sleeve * 1.25)], crate::draw::MUTED, 1.5)
            .note((r * 2.0, -g - sleeve / 2.0), "sleeve, open at the bottom", SILVER, Anchor::Start)
            .note((0.0, -g - sleeve * 1.3), "coax to the radio", crate::draw::MUTED, Anchor::Middle)
            .dim((-4.0 * r, g), (-4.0 * r, g + top), "radiator", (-8.0, 4.0), Anchor::End)
            .dim((-4.0 * r, -g), (-4.0 * r, -g - sleeve), "sleeve", (-8.0, 4.0), Anchor::End)
            .caption("equal in all horizontal directions", GREEN)
            .tall(420.0)
            .render(),
        feed: vertical(
            "centre conductor, carrying on up as the radiator",
            "braid, soldered all round to the top of the sleeve",
            Some("The coax runs down inside the sleeve and out of its open bottom end."),
        ),
        notes: "**The marine and AIS classic.** Strip a quarter wave of jacket and braid off \
                the end of the coax and the centre conductor becomes the top half. Fold the \
                braid back over the jacket, or slide a tube over it, and connect it at the top: \
                that is the bottom half.\n\n**The sleeve does two jobs.** It radiates as the \
                lower half of a dipole, and because it is a quarter wave long and open at the \
                bottom it looks like a high impedance to current trying to leave down the \
                outside of the coax. That makes it its own choke, which is why it tolerates the \
                coax running straight down through it. A ferrite below the sleeve still helps.\
                \n\nThe model builds the sleeve as a cage of six wires; a solid tube behaves the \
                same. A fat sleeve needs to be a little shorter than the radiator. About 70 Ω, \
                1.4:1 on 50 Ω coax."
            .into(),
        cut: None,
    }
}
