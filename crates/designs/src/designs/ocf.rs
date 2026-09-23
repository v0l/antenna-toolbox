use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static OCF: Design = Design {
    id: "ocf",
    name: "Off-centre-fed dipole",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "2.2 dBi",
    controls: &[],
    polarisation: "**Linear, along the wire.** A dipole's figure-eight on its fundamental. \
                   Moving the feed changes the impedance, not the pattern.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let len = c.P("length", 0.482 * lam);
    let short = len / 3.0;
    let long = len - short;
    Output {
        spec: "~2 dBi · about 100 Ω here, 200 to 300 Ω on the harmonics".into(),
        rows: vec![
            row("**short leg**, a third of the length", c.fmt(short)),
            row("**long leg**, two thirds", c.fmt(long)),
            total("total wire", c.fmt(len)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[-short, 0.0, 0.0], [-0.01 * len, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[0.01 * len, 0.0, 0.0], [long, 0.0, 0.0]], GOLD, 3.5),
            ],
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([(long - short) / 2.0, 0.0, 0.0]),
            pol: "polarisation: linear, along the wire".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![vec![[-short, 0.0, 0.0], [long, 0.0, 0.0]]],
            [0.0; 3],
        )),
        diagram: Sketch::new()
            .wire(&[(-short, 0.0), (long, 0.0)], GOLD, 3.0)
            .dim((-short, 0.05 * len), (0.0, 0.05 * len), "⅓", (0.0, -8.0), Anchor::Middle)
            .dim((0.0, 0.05 * len), (long, 0.05 * len), "⅔", (0.0, -8.0), Anchor::Middle)
            .caption("broadside to the wire, nulls off the ends", GREEN)
            .feed((0.0, 0.0), true)
            .render(),
        feed: inline("short leg", "long leg", Some("Use a 4:1 current balun, not a voltage balun.")),
        notes: "**The Windom, or OCF.** A half-wave wire fed a third of the way along instead \
                of in the middle. At that point the impedance is about 100 Ω on the \
                fundamental, which is what is solved here, and 150 to 300 Ω on the even \
                harmonics, where the same point is also away from a current null. That is \
                why HF operators use one wire for 80, 40, 20 and 10 m through a 4:1 balun, a \
                compromise across all of them rather than a match on any one.\n\n**The feed is unbalanced by design.** The two legs are different \
                lengths, so common-mode current on the coax is a real problem. A good current \
                balun at the feed, and a second choke a quarter wave down the coax, are part \
                of the antenna rather than extras."
            .into(),
        cut: None,
    }
}
