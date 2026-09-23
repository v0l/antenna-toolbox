use crate::draw::{Anchor, BOOM, GOLD, GREEN, PALE, SILVER};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, Load, SolveLine, WireGeometry};

pub static HB9CV: Design = Design {
    id: "hb9cv",
    name: "HB9CV",
    group: Group::Beam,
    build: Build::Wire,
    gain: "6.6 dBi",
    controls: &[],
    polarisation: "**Linear, along the elements.** A two-element beam with both elements \
                   driven, so it keeps a good front to back over a wider band than a \
                   two-element Yagi.",
    compute,
};

pub const CAP_OHMS: f64 = 92.0;

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let front = c.P("radiator", 0.4554 * lam);
    let rear = c.P("reflector", 0.4916 * lam);
    let s = c.P("spacing", 0.1253 * lam);
    let t = c.P("tap", 0.0916 * lam);
    let d = c.P("rod above boom", 0.0065 * lam);
    let boom_r = 0.0036 * lam;
    let f_hz = 299_792_458.0 / (lam / 1000.0);
    let cap_pf = 1e12 / (2.0 * std::f64::consts::PI * f_hz * CAP_OHMS);
    let rod = [[-t, 0.0, 0.0], [-t, d, 0.0], [0.0, d, 0.0], [0.0, d, -s], [t, d, -s], [t, 0.0, -s]];
    let fine = |a: [f64; 3], b: [f64; 3]| {
        let l = antenna_solver::vec::length(antenna_solver::vec::sub(b, a));
        Some(((l / (0.004 * lam)).ceil() as usize).max(1))
    };
    let mut geo = WireGeometry::new(
        vec![
            vec![
                [-front / 2.0, 0.0, 0.0],
                [-t, 0.0, 0.0],
                [0.0; 3],
                [t, 0.0, 0.0],
                [front / 2.0, 0.0, 0.0],
            ],
            vec![
                [-rear / 2.0, 0.0, -s],
                [-t, 0.0, -s],
                [0.0, 0.0, -s],
                [t, 0.0, -s],
                [rear / 2.0, 0.0, -s],
            ],
        ],
        [0.0, d / 2.0, 0.0],
    );
    for w in rod.windows(2) {
        geo.lines.push(SolveLine {
            pts: vec![w[0], w[1]],
            rad: None,
            props: Default::default(),
            segments: fine(w[0], w[1]),
        });
    }
    geo.lines.push(SolveLine {
        pts: vec![[0.0; 3], [0.0, 0.0, -s]],
        rad: Some(boom_r),
        props: Default::default(),
        segments: fine([0.0; 3], [0.0, 0.0, -s]),
    });
    geo.lines.push(vec![[0.0; 3], [0.0, d, 0.0]].into());
    geo.loads.push(([0.0, d / 2.0, 0.0], Load::Series { r: 0.0, l: 0.0, c: cap_pf * 1e-12 }));
    Output {
        spec: "~6.6 dBi · 18 dB F/B · both elements driven through a rod over the boom".into(),
        rows: vec![
            row("**radiator**, front element, tip to tip", c.fmt(front)),
            row("**reflector**, rear element, tip to tip", c.fmt(rear)),
            row("**spacing**, centre to centre", c.fmt(s)),
            row("**phasing rod** taps, from the boom on each element", c.fmt(t)),
            row("rod above the boom, centre to centre", c.fmt(d)),
            row("series capacitor at the feed", format!("{cap_pf:.1} pF")),
            total("boom length, a little over", c.fmt(s)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[0.0; 3], [0.0, 0.0, -s]], BOOM, 5.0),
                wire(vec![[-front / 2.0, 0.0, 0.0], [front / 2.0, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[-rear / 2.0, 0.0, -s], [rear / 2.0, 0.0, -s]], SILVER, 3.5),
                wire(rod.to_vec(), GREEN, 2.0),
            ],
            feed: Some([0.0, d / 2.0, 0.0]),
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
            .wire(
                &[
                    (-t, 0.0),
                    (-t, 0.012 * lam),
                    (0.012 * lam, 0.012 * lam),
                    (0.012 * lam, -s - 0.012 * lam),
                    (t, -s - 0.012 * lam),
                    (t, -s),
                ],
                GREEN,
                1.5,
            )
            .note((front / 2.0 + 0.02 * lam, 0.0), "radiator", GOLD, Anchor::Start)
            .note((rear / 2.0 + 0.02 * lam, -s), "reflector", PALE, Anchor::Start)
            .note((0.03 * lam, -s / 2.0), "phasing rod over the boom", GREEN, Anchor::Start)
            .dim((-t, 0.04 * lam), (0.0, 0.04 * lam), "tap", (0.0, -8.0), Anchor::Middle)
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
        feed: inline(
            "boom and element centre, on the shield",
            "phasing rod at the front, through the capacitor",
            None,
        ),
        notes: "**Two driven elements an eighth of a wave apart.** Both elements are solid and \
                bolted through a metal boom. A single rod runs 5 mm above the boom, tapped onto \
                the radiator on one side and the reflector on the other; with the boom it forms \
                a transmission line, and swapping sides reverses the phase. The rear element ends \
                up fed about 225° behind the front, which cancels the back and adds forward. \
                Small, light, and much less fussy about element diameter than a two-element \
                Yagi, which is why it is the classic fox-hunting beam.\n\n**Dimensions are \
                DK7ZB's 2 m version**, scaled to the design frequency: 945 mm and 1020 mm \
                elements 260 mm apart, taps 190 mm out, 2 mm rod, 12 pF at the feed. Solved as \
                built it gives 6.6 dBi and 18 dB front to back, in line with published \
                measurements, but the impedance comes out near 140 Ω where DK7ZB measured 50. \
                The taps behave like gamma matches and are very sensitive to the rod's spacing \
                and the boom's shape, so treat the tap and capacitor as the adjustments they are \
                on the bench: moving the taps in to about 0.04 λ brings the model to 50 Ω, at \
                some cost in front to back."
            .into(),
        cut: None,
    }
}
