use crate::draw::{Anchor, Drawing, GOLD, GREY, INK, PALE, SILVER};
use crate::feed_detail::inline;
use crate::{Build, Ctx, Design, Group, MOXON as R, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static MOXON: Design = Design {
    id: "moxon",
    name: "Moxon rectangle",
    group: Group::Beam,
    build: Build::Wire,
    gain: "6 dBi",
    controls: &[],
    polarisation: "**Linear, parallel to dimension A.** Hold the rectangle with A horizontal and \
                   you have a horizontally polarised beam; stand it on end for vertical. Bandwidth \
                   is narrow, roughly 2 to 4 percent, so cut it for the frequency you actually want.",
    compute,
};

fn diagram(a: f64, b: f64, cg: f64, d: f64) -> Drawing {
    let w = 640.0;
    let pad = 78.0;
    let s = (w - 2.0 * pad) / a;
    let h = (b + cg + d) * s + 2.0 * pad;
    let (x0, x1) = (pad, pad + a * s);
    let ax = (x0 + x1) / 2.0;
    let yd = pad;
    let ydt = pad + b * s;
    let yrt = ydt + cg * s;
    let yr = yrt + d * s;
    let mut g = Drawing::new(w, h);
    g.polyline(&[(x0, ydt), (x0, yd), (x1, yd), (x1, ydt)], GOLD, 3.5);
    g.polyline(&[(x0, yrt), (x0, yr), (x1, yr), (x1, yrt)], PALE, 3.5);
    g.rect(ax - 5.0, yd - 4.0, 10.0, 8.0, Some(INK), None);
    g.dot(ax - 4.0, yd, 2.6, GOLD);
    g.dot(ax + 4.0, yd, 2.6, GOLD);
    g.feed_flag(ax, yd, false);
    g.dim(x0, yr + 30.0, x1, yr + 30.0, "A", 0.0, -6.0, Anchor::Middle);
    g.dim(x0 - 26.0, yd, x0 - 26.0, ydt, "B", -12.0, 5.0, Anchor::Middle);
    g.dim(x0 - 26.0, ydt, x0 - 26.0, yrt, "C", -12.0, 5.0, Anchor::Middle);
    g.dim(x0 - 26.0, yrt, x0 - 26.0, yr, "D", -12.0, 5.0, Anchor::Middle);
    g.dim(x1 + 30.0, yd, x1 + 30.0, yr, "E", 16.0, 5.0, Anchor::Middle);
    g.text_at(x1 - 8.0, yd - 10.0, "driven", GOLD, 12.0, Anchor::End, crate::draw::Face::Sans);
    g.text_at(x1 - 8.0, yr + 18.0, "reflector", GREY, 12.0, Anchor::End, crate::draw::Face::Sans);
    g.beam_label(ax, h - 14.0, None);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let a = c.P("A width", R[0] * lam);
    let b = c.P("B driven tail", R[1] * lam);
    let cg = c.P("C tip gap", R[2] * lam);
    let d = c.P("D refl tail", R[3] * lam);
    let thick = c.wire_dia / lam > 1.5e-3;

    let driven = vec![[-a / 2.0, 0.0, -b], [-a / 2.0, 0.0, 0.0], [a / 2.0, 0.0, 0.0], [a / 2.0, 0.0, -b]];
    let refl = vec![
        [-a / 2.0, 0.0, -(b + cg)],
        [-a / 2.0, 0.0, -(b + cg + d)],
        [a / 2.0, 0.0, -(b + cg + d)],
        [a / 2.0, 0.0, -(b + cg)],
    ];

    Output {
        spec: "~6 dBi · 20 dB F/B · 50 Ω direct".into(),
        rows: vec![
            row("**A** width, both elements", c.fmt(a)),
            row("**B** driven tails (each end)", c.fmt(b)),
            row("**C** gap between tail tips", c.fmt(cg)),
            row("**D** reflector tails (each end)", c.fmt(d)),
            row("**E** total depth, B+C+D", c.fmt(b + cg + d)),
            total("driven wire, cut A + 2B", c.fmt(a + 2.0 * b)),
            total("reflector wire, cut A + 2D", c.fmt(a + 2.0 * d)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[-a / 2.0, 0.0, -b], [-a / 2.0, 0.0, 0.0], [-a * 0.02, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[a * 0.02, 0.0, 0.0], [a / 2.0, 0.0, 0.0], [a / 2.0, 0.0, -b]], GOLD, 3.5),
                wire(refl.clone(), SILVER, 3.5),
            ],
            feed: Some([0.0; 3]),
            beam_from: Some(0.0),
            pol: "polarisation: linear, along the A dimension".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(vec![driven, refl], [0.0; 3])),
        diagram: diagram(a, b, cg, d),
        feed: inline(
            "driven element, left half",
            "driven element, right half",
            Some("Gap C at the folded tips is a separate dimension. Do not confuse the two."),
        ),
        notes: format!(
            "**Best gain per unit of effort.** One bent wire for the reflector, one for the driven \
             element, no matching network, near 50 Ω on its own. Tails bend toward each other, \
             the reflector connects to nothing, the beam fires away from it.\n\n**Choke the \
             feedline.** Balanced antenna on unbalanced coax, so the braid radiates and smears the \
             pattern. Ferrite bead at the feedpoint or 4-6 tight turns of coax behind it.\n\n{}These \
             are thin-wire ratios and the real dimensions drift a few percent with wire diameter. \
             The solver uses your actual wire, so trust its numbers over the table when the two \
             disagree. Trim A for resonance, adjust C for front-to-back.",
            if thick { "^^Your wire is thick relative to λ.^^ " } else { "" }
        ),
        cut: None,
    }
}
