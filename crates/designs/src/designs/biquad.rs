use crate::draw::{Anchor, DIM, Drawing, GOLD, GREY, INK, METAL, MUTED, Stroke, hex};
use crate::feed_detail::FeedDetail;
use crate::{Build, Ctx, Design, Group, Output, Poly, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use std::f64::consts::SQRT_2;

pub static BIQUAD: Design = Design {
    id: "biquad",
    name: "Biquad",
    group: Group::Beam,
    build: Build::Wire,
    gain: "11 dBi",
    controls: &[],
    polarisation: "**Linear, perpendicular to the stacking axis.** Drawn here with the two \
                   diamonds stacked vertically, which gives horizontal polarisation. Rotate the \
                   whole element 90° so the diamonds sit side by side and it turns vertical, which \
                   is what you want against most WiFi access points. Usefully wide-band, on the \
                   order of 15 percent.",
    compute,
};

fn diagram(diag: f64, plate: f64, gap: f64) -> Drawing {
    let w = 640.0;
    let pad = 46.0;
    let sc = (w * 0.46 / plate).min(250.0 / (2.0 * diag));
    let cx = w * 0.30;
    let cy = pad + plate * sc / 2.0;
    let p = plate * sc / 2.0;
    let d = diag * sc;
    let h = cy + p + 60.0;
    let vx = w * 0.76;
    let vg = gap * sc;
    let mut g = Drawing::new(w, h);
    g.rect(cx - p, cy - p, 2.0 * p, 2.0 * p, Some(hex(0x1b2024)), Some(Stroke::new(METAL, 2.0)));
    for yc in [cy - d / 2.0, cy + d / 2.0] {
        let half = d / 2.0;
        g.polygon(
            &[(cx, yc - half), (cx + half, yc), (cx, yc + half), (cx - half, yc)],
            None,
            Some(Stroke::new(GOLD, 3.5)),
        );
    }
    g.rect(cx - 6.0, cy - 7.0, 12.0, 14.0, Some(INK), None);
    g.dot(cx, cy - 4.0, 2.8, GOLD);
    g.dot(cx, cy + 4.0, 2.8, GREY);
    g.label(cx + 12.0, cy - 4.0, "pin", GOLD, 11.0, Anchor::Start);
    g.label(cx + 12.0, cy + 9.0, "shield", GREY, 11.0, Anchor::Start);
    g.label(cx, cy - p - 10.0, "reflector plate, ≥ 1.1 λ square", GREY, 12.0, Anchor::Middle);
    g.label(cx, cy + p + 22.0, "front view", GOLD, 12.0, Anchor::Middle);
    g.line(vx, cy - p, vx, cy + p, METAL, 5.0);
    g.line(vx + vg, cy - d, vx + vg, cy - 4.0, GOLD, 3.0);
    g.line(vx + vg, cy + 4.0, vx + vg, cy + d, GOLD, 3.0);
    g.rect(vx - 22.0, cy - 7.0, 30.0, 14.0, Some(hex(0x3a4349)), Some(Stroke::new(METAL, 1.2)));
    g.line(vx + 4.0, cy - 4.0, vx + vg, cy - 4.0, GOLD, 2.4);
    g.line(vx, cy + 4.0, vx + vg, cy + 4.0, GREY, 2.4);
    g.dashed(vx, cy - p - 8.0, vx + vg, cy - p - 8.0, DIM, 3.0, 3.0);
    g.mono(vx + vg / 2.0, cy - p - 14.0, "gap", DIM, 12.0, Anchor::Middle);
    g.label(vx - 28.0, cy + 4.0, "coax", MUTED, 12.0, Anchor::End);
    g.label(vx, cy + p + 22.0, "side view", GREY, 12.0, Anchor::Middle);
    g.beam_label(w / 2.0, h - 14.0, Some("beam fires out of the plate, toward the viewer"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let side = c.P("side", 0.25 * lam);
    let diag = side * SQRT_2;
    let plate = c.P("plate side", 1.1 * lam);
    let gap = c.P("gap to plate", 0.12 * lam);
    let hd = diag / 2.0;
    let hp = plate / 2.0;
    let e = lam / 120.0;
    let gg = lam / 120.0;
    Output {
        spec: "~11 dBi · needs a reflector plate · ~50 Ω".into(),
        rows: vec![
            row("**side** length, each of 8", c.fmt(side)),
            row("element width (one diagonal)", c.fmt(diag)),
            row("element height (two diagonals)", c.fmt(2.0 * diag)),
            row("**plate**, square side, minimum", c.fmt(plate)),
            row("**gap** element to plate", c.fmt(gap)),
            total("total wire, one piece", c.fmt(8.0 * side)),
        ],
        scene: Scene {
            wires: vec![
                wire(
                    vec![
                        [0.0, hd * 0.07, 0.0],
                        [-hd, hd, 0.0],
                        [0.0, 2.0 * hd, 0.0],
                        [hd, hd, 0.0],
                        [0.0, hd * 0.07, 0.0],
                    ],
                    GOLD,
                    3.5,
                ),
                wire(
                    vec![
                        [0.0, -hd * 0.07, 0.0],
                        [-hd, -hd, 0.0],
                        [0.0, -2.0 * hd, 0.0],
                        [hd, -hd, 0.0],
                        [0.0, -hd * 0.07, 0.0],
                    ],
                    GOLD,
                    3.5,
                ),
            ],
            polys: vec![Poly {
                p: vec![[-hp, -hp, -gap], [hp, -hp, -gap], [hp, hp, -gap], [-hp, hp, -gap]],
                fill: None,
                stroke: None,
            }],
            feed: Some([0.0; 3]),
            beam_from: Some(0.0),
            pol: "polarisation: linear, across the diamonds".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(
            WireGeometry::new(
                vec![
                    vec![[-e, gg, 0.0], [-hd, hd, 0.0], [0.0, 2.0 * hd, 0.0], [hd, hd, 0.0], [e, gg, 0.0]],
                    vec![[e, -gg, 0.0], [hd, -hd, 0.0], [0.0, -2.0 * hd, 0.0], [-hd, -hd, 0.0], [-e, -gg, 0.0]],
                    vec![[e, gg, 0.0], [e, -gg, 0.0]],
                    vec![[-e, gg, 0.0], [-e, -gg, 0.0]],
                ],
                [-e, 0.0, 0.0],
            )
            .ground(-gap),
        ),
        diagram: diagram(diag, plate, gap),
        feed: FeedDetail::Plate,
        notes: "**The 2.4 GHz workhorse, and the only design here that needs more than wire.** \
                One continuous piece bent into two diamonds that meet at the centre. The two centre \
                points are where the SMA goes: one to the pin, one to the shield. The open ends of \
                the wire meet at the centre too, so eight equal sides, one cut, two joints.\n\n\
                **The plate is not optional.** Without it you have a figure-eight pattern and no \
                gain. Any sheet metal works: copper clad, a tin lid, an old hard drive cover. \
                Bigger than 1.1 λ square helps front-to-back a little. Bending 10 mm lips up along \
                the two side edges adds about 1 dB.\n\nThe gap sets the feed impedance, and in this model, fed across a small gap at the centre, \
                it comes out well below 50 Ω and capacitive. NEC-2 agrees on the same geometry. \
                Real builds trim the gap and the side length while watching SWR, so treat the tune \
                button and the match panel as the starting point."
            .into(),
        cut: None,
    }
}
