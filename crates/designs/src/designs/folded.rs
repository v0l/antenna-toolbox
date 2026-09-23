use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static FOLDED: Design = Design {
    id: "folded",
    name: "Folded dipole",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "2.2 dBi",
    controls: &[],
    polarisation: "**Linear, along the elements.** Exactly a dipole's pattern. Folding changes \
                   the impedance, not where it points.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let len = c.P("length", 0.459 * lam);
    let s = c.P("spacing", 0.02 * lam);
    let h = len / 2.0;
    Output {
        spec: "~2 dBi · 280 Ω, 4:1 balun to 75 Ω or 300 Ω twin lead".into(),
        rows: vec![
            row("**length**, end to end", c.fmt(len)),
            row("**spacing** between the two wires, centre to centre", c.fmt(s)),
            row("fed wire, each half from the gap", c.fmt(h)),
            total("total wire", c.fmt(2.0 * len + 2.0 * s)),
        ],
        scene: Scene {
            wires: vec![wire(
                vec![
                    [-0.02 * len, 0.0, 0.0],
                    [-h, 0.0, 0.0],
                    [-h, s, 0.0],
                    [h, s, 0.0],
                    [h, 0.0, 0.0],
                    [0.02 * len, 0.0, 0.0],
                ],
                GOLD,
                3.5,
            )],
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pol: "polarisation: linear, along the elements".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![vec![
                [0.0; 3],
                [-h, 0.0, 0.0],
                [-h, s, 0.0],
                [h, s, 0.0],
                [h, 0.0, 0.0],
                [0.0; 3],
            ]],
            [0.0; 3],
        )),
        diagram: Sketch::new()
            .wire(
                &[(-0.02 * len, 0.0), (-h, 0.0), (-h, s), (h, s), (h, 0.0), (0.02 * len, 0.0)],
                GOLD,
                3.0,
            )
            .dim((-h, -0.08 * len), (h, -0.08 * len), "length", (0.0, 14.0), Anchor::Middle)
            .dim((h + 0.03 * len, 0.0), (h + 0.03 * len, s), "spacing", (8.0, 4.0), Anchor::Start)
            .caption("broadside to the wire, nulls off the ends", GREEN)
            .feed((0.0, 0.0), true)
            .render(),
        feed: inline("left half of the fed wire", "right half of the fed wire", None),
        notes: "**A half-wave dipole with a second wire alongside, joined at both ends.** The \
                current splits equally between the two wires, so the feed sees half the \
                current for the same radiated power, and four times the impedance: about \
                280 Ω instead of 73.\n\n**Why bother.** That impedance suits 300 Ω twin lead \
                directly, or a 4:1 balun into 75 Ω coax, and it is the usual driven element in \
                Yagis, where the parasitic elements drag a plain dipole's impedance down. It \
                is also a little broader in bandwidth than a thin dipole, and the whole \
                element is at DC ground through the fold, which bleeds off static.\n\n\
                Spacing is not critical; wider spacing shortens the resonant length slightly."
            .into(),
        cut: None,
    }
}
