use crate::draw::{Anchor, BOOM, GOLD, GREEN, PALE, SILVER};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::C64;
use antenna_solver::geometry::{Geometry, Network, WireGeometry};

pub static HB9CV: Design = Design {
    id: "hb9cv",
    name: "HB9CV",
    group: Group::Beam,
    build: Build::Wire,
    gain: "6.4 dBi",
    controls: &[],
    polarisation: "**Linear, along the elements.** A two-element beam with both elements \
                   driven, so it keeps a good front to back over a wider band than a \
                   two-element Yagi.",
    compute,
};

pub const LINE_OHMS: f64 = 50.0;

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let front = c.P("front", 0.46 * lam);
    let rear = c.P("rear", 0.49 * lam);
    let s = c.P("spacing", 0.125 * lam);
    let mut geo = WireGeometry::new(
        vec![
            vec![[-front / 2.0, 0.0, 0.0], [front / 2.0, 0.0, 0.0]],
            vec![[-rear / 2.0, 0.0, -s], [rear / 2.0, 0.0, -s]],
        ],
        [0.0; 3],
    );
    geo.networks.push((
        [0.0; 3],
        [0.0, 0.0, -s],
        Network::Line { z0: LINE_OHMS, length: s, crossed: true, shunt: [C64::new(0.0, 0.0); 2] },
    ));
    Output {
        spec: "~6.4 dBi · deep null behind · about 13 Ω, both elements driven".into(),
        rows: vec![
            row("**front element**, tip to tip", c.fmt(front)),
            row("**rear element**, tip to tip", c.fmt(rear)),
            row("**spacing**", c.fmt(s)),
            row("phasing line, crossed, between the element centres", format!("{LINE_OHMS:.0} Ω")),
            total("boom length", c.fmt(s)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[0.0; 3], [0.0, 0.0, -s]], BOOM, 5.0),
                wire(vec![[-front / 2.0, 0.0, 0.0], [-0.01 * lam, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[0.01 * lam, 0.0, 0.0], [front / 2.0, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[-rear / 2.0, 0.0, -s], [-0.01 * lam, 0.0, -s]], SILVER, 3.5),
                wire(vec![[0.01 * lam, 0.0, -s], [rear / 2.0, 0.0, -s]], SILVER, 3.5),
                wire(
                    vec![[-0.01 * lam, 0.01 * lam, 0.0], [0.01 * lam, 0.01 * lam, -s]],
                    GREEN,
                    1.5,
                ),
                wire(
                    vec![[0.01 * lam, 0.01 * lam, 0.0], [-0.01 * lam, 0.01 * lam, -s]],
                    GREEN,
                    1.5,
                ),
            ],
            feed: Some([0.0; 3]),
            beam_from: Some(0.0),
            pattern_origin: Some([0.0, 0.0, -s / 2.0]),
            pol: "polarisation: linear, along the elements".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(geo),
        diagram: Sketch::new()
            .wire(&[(0.0, 0.0), (0.0, -s)], BOOM, 6.0)
            .wire(&[(-front / 2.0, 0.0), (front / 2.0, 0.0)], GOLD, 3.0)
            .wire(&[(-rear / 2.0, -s), (rear / 2.0, -s)], PALE, 3.0)
            .wire(&[(-0.01 * lam, 0.0), (0.01 * lam, -s)], GREEN, 1.5)
            .wire(&[(0.01 * lam, 0.0), (-0.01 * lam, -s)], GREEN, 1.5)
            .note((front / 2.0 + 0.02 * lam, 0.0), "front", GOLD, Anchor::Start)
            .note((rear / 2.0 + 0.02 * lam, -s), "rear", PALE, Anchor::Start)
            .dim(
                (-rear / 2.0 - 0.04 * lam, 0.0),
                (-rear / 2.0 - 0.04 * lam, -s),
                "spacing",
                (-8.0, 4.0),
                Anchor::End,
            )
            .caption("↑ beam direction ↑", GREEN)
            .feed((0.0, 0.0), false)
            .render(),
        feed: inline("front element, one side", "front element, other side", None),
        notes: "**Two driven elements an eighth of a wave apart, joined by a crossed line.** \
                The crossing adds 180° and the spacing another 45°, so the rear element is \
                fed 225° behind the front one, which cancels the back and adds forward. Popular \
                for fox hunting and portable work because it is small, light, and not fussy \
                about element diameter.\n\n**The model is the ideal version.** It joins the \
                element centres with a crossed 50 Ω line of the same electrical length as the \
                spacing, which gives the deepest null behind and about 13 Ω at the feed; a \
                quarter wave of 25 Ω (two 50 Ω coax in parallel) brings that to 50. Real \
                HB9CVs make the line from two rods along the boom, tapped onto the elements a \
                little way out from the centre like a gamma match, with a small series \
                capacitor at the feed. That raises the impedance to 50 Ω directly; expect to \
                adjust the tap points and capacitor on the bench."
            .into(),
        cut: None,
    }
}
