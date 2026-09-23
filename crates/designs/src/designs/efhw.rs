use crate::draw::{Anchor, GOLD, GREEN, SILVER};
use crate::feed_detail::FeedDetail;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static EFHW: Design = Design {
    id: "efhw",
    name: "End-fed half-wave",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "2.2 dBi",
    controls: &[],
    polarisation: "**Linear, along the wire.** Same figure-eight as a centre-fed dipole: most \
                   of the current sits in the middle of the wire whichever end you feed it from.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let len = c.P("radiator", 0.451 * lam);
    let cp = c.P("counterpoise", 0.1 * lam);
    let g = lam / 400.0;
    Output {
        spec: "~2 dBi · linear · 1 to 3 kΩ at the end, into a step-up transformer".into(),
        rows: vec![
            row("**radiator**, from the transformer to the far end", c.fmt(len)),
            row("**counterpoise**, or the coax braid", c.fmt(cp)),
            row("transformer, impedance ratio", "end impedance over 50 Ω"),
            row("49:1, for about 2.5 kΩ", "2 turns to 14 on a type 43 toroid"),
            row("25:1, for about 1.2 kΩ", "2 turns to 10"),
            total("total wire", c.fmt(len + cp)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[g, 0.0, 0.0], [g + len, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[-g, 0.0, 0.0], [-g - cp, 0.0, 0.0]], SILVER, 2.5),
            ],
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([len / 2.0, 0.0, 0.0]),
            pol: "polarisation: linear, along the wire".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![vec![[-g - cp, 0.0, 0.0], [-g, 0.0, 0.0], [g, 0.0, 0.0], [g + len, 0.0, 0.0]]],
            [0.0; 3],
        )),
        diagram: Sketch::new()
            .wire(&[(g, 0.0), (g + len, 0.0)], GOLD, 3.0)
            .wire(&[(-g - cp, 0.0), (-g, 0.0)], SILVER, 2.5)
            .dim((g, 0.06 * len), (g + len, 0.06 * len), "radiator", (0.0, -8.0), Anchor::Middle)
            .note((-g - cp / 2.0, -0.05 * len), "counterpoise", SILVER, Anchor::Middle)
            .note((0.0, -0.1 * len), "step-up transformer", GREEN, Anchor::Middle)
            .caption("broadside to the wire, nulls off the ends", GREEN)
            .render(),
        feed: FeedDetail::Transformer,
        notes: "**A half-wave wire fed at one end.** Popular for portable and HF work because \
                there is only one support point near the radio and the wire can go up at any \
                angle. At the end of a half wave the voltage is high and the current low, so \
                the impedance is high: 2 to 3 kΩ for thin wire on HF, which is why the usual \
                transformer is 49:1 (2 turns to 14 on a ferrite toroid). Thicker wire relative \
                to the wavelength lowers it; this 2 mm wire at VHF solves to about 1.1 kΩ, \
                which wants 25:1 instead.\n\n\
                **The counterpoise is not optional.** Something has to carry the return \
                current. A short wire or, more often, the outside of the coax does it. The \
                model uses a 0.1 λ wire, long enough that the end resonates; with coax \
                instead, put a choke further down so the counterpoise length is defined.\n\nThe impedance at the end is very sensitive to \
                length, so the SWR on 50 Ω at the top is the raw end impedance before the \
                transformer. The matching section picks the ratio from the solved end \
                impedance."
            .into(),
        cut: None,
    }
}
