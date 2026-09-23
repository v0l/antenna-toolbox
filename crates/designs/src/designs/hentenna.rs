use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static HENTENNA: Design = Design {
    id: "hentenna",
    name: "Hentenna",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "5.3 dBi",
    controls: &[],
    polarisation: "**Horizontal when it stands upright**, even though it is tall and narrow: \
                   the currents that radiate run across the short top and bottom wires. It beams \
                   broadside, front and back.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let hgt = c.P("height", 0.53 * lam);
    let w = c.P("width", 0.145 * lam);
    let f = c.P("feed bar", 0.135 * lam);
    let x = w / 2.0;
    let lines = vec![
        vec![[-x, f, 0.0], [-x, 0.0, 0.0], [x, 0.0, 0.0], [x, f, 0.0]],
        vec![[-x, f, 0.0], [-x, hgt, 0.0], [x, hgt, 0.0], [x, f, 0.0]],
        vec![[-x, f, 0.0], [x, f, 0.0]],
    ];
    Output {
        spec: "~5 dBi · horizontal · about 50 Ω at the bar".into(),
        rows: vec![
            row("**height**", c.fmt(hgt)),
            row("**width**", c.fmt(w)),
            row("**feed bar**, above the bottom wire", c.fmt(f)),
            total("total wire", c.fmt(2.0 * hgt + 3.0 * w)),
        ],
        scene: Scene {
            wires: vec![
                wire(
                    vec![
                        [-x, 0.0, 0.0],
                        [x, 0.0, 0.0],
                        [x, hgt, 0.0],
                        [-x, hgt, 0.0],
                        [-x, 0.0, 0.0],
                    ],
                    GOLD,
                    3.5,
                ),
                wire(vec![[-x, f, 0.0], [-0.1 * w, f, 0.0]], GOLD, 2.5),
                wire(vec![[0.1 * w, f, 0.0], [x, f, 0.0]], GOLD, 2.5),
            ],
            feed: Some([0.0, f, 0.0]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([0.0, hgt / 2.0, 0.0]),
            pol: "polarisation: horizontal".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(lines, [0.0, f, 0.0])),
        diagram: Sketch::new()
            .wire(&[(-x, 0.0), (x, 0.0), (x, hgt), (-x, hgt), (-x, 0.0)], GOLD, 3.0)
            .wire(&[(-x, f), (x, f)], GOLD, 2.0)
            .dim((-x - 0.08 * hgt, 0.0), (-x - 0.08 * hgt, hgt), "height", (-8.0, 4.0), Anchor::End)
            .dim(
                (-x, hgt + 0.05 * hgt),
                (x, hgt + 0.05 * hgt),
                "width",
                (0.0, -8.0),
                Anchor::Middle,
            )
            .dim((x + 0.08 * hgt, 0.0), (x + 0.08 * hgt, f), "bar", (8.0, 4.0), Anchor::Start)
            .caption("broadside to the rectangle, front and back", GREEN)
            .tall(380.0)
            .feed((0.0, f), false)
            .render(),
        feed: inline("left half of the bar", "right half of the bar", None),
        notes: "**A Japanese design from the 1970s, a half-wave tall rectangle with a feed \
                bar near the bottom.** It matches 50 Ω coax without a balun or matching \
                network: sliding the bar up or down sets the impedance, and changing the \
                width sets resonance.\n\n**Better than a dipole, by about 3 dB,** because the \
                top wire and the bottom section act like two stacked horizontal dipoles fed \
                in phase. It is narrow-band, so build it slightly long and tune the bar \
                position with an analyser.\n\nTurn it on its side, long wires horizontal, \
                and the polarisation turns vertical with the same gain."
            .into(),
        cut: None,
    }
}
