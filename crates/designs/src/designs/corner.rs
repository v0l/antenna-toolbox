use crate::draw::{Anchor, Drawing, GOLD, GREY, SILVER, hexa};
use crate::feed_detail::inline;
use crate::feeds::{FeedFrame, FeedKind, build_feed};
use crate::{Build, ControlId, CutFile, Ctx, DIPOLE_K, Design, Group, Output, Poly, Scene, row, total};
use antenna_solver::geometry::{Geometry, ImagePlane, WireGeometry};
use antenna_solver::vec::Vec3;
use std::f64::consts::FRAC_1_SQRT_2 as ROOT_HALF;
use std::sync::Arc;

pub static CORNER: Design = Design {
    id: "corner",
    name: "Corner reflector",
    group: Group::Beam,
    build: Build::Both,
    gain: "12 dBi",
    controls: &[ControlId::FeedType],
    polarisation: "**Linear, along the driven element.** The corner does not change polarisation, \
                   it just throws everything forward. Mount the dipole parallel to the apex line, \
                   which is how the images line up, and the pattern is a single clean forward lobe \
                   with very little behind it.",
    compute,
};

fn diagram(apex: f64, side: f64, dipole: f64) -> Drawing {
    let (w, h) = (640.0, 330.0);
    let cx = 150.0;
    let cy = h / 2.0;
    let sc = ((w - 260.0) / (side * ROOT_HALF)).min((h - 90.0) / (side * 2.0 * ROOT_HALF));
    let arm = side * sc;
    let dx = apex * sc * ROOT_HALF;
    let mut g = Drawing::new(w, h);
    g.line(cx, cy, cx + arm * ROOT_HALF, cy - arm * ROOT_HALF, SILVER, 4.0);
    g.line(cx, cy, cx + arm * ROOT_HALF, cy + arm * ROOT_HALF, SILVER, 4.0);
    g.line(cx + dx - 2.0, cy - dipole * sc / 2.0, cx + dx - 2.0, cy + dipole * sc / 2.0, GOLD, 4.0);
    g.feed_flag(cx + dx, cy, false);
    g.dim(cx, cy + 40.0, cx + dx, cy + 40.0, "apex to driven", 0.0, -8.0, Anchor::Middle);
    g.label(cx + arm * ROOT_HALF + 10.0, cy - arm * ROOT_HALF, "plate, side length", SILVER, 12.0, Anchor::Start);
    g.label(cx - 10.0, cy + 4.0, "apex, 90°", GREY, 12.0, Anchor::End);
    g.beam_label(w - 120.0, cy, Some("→ beam"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let driven = c.P("driven", DIPOLE_K * lam);
    let apex = c.P("apex to driven", 0.5 * lam);
    let side = c.P("plate side", 1.2 * lam);
    let kind = FeedKind::from_control(c.ctl(ControlId::FeedType));
    let cx = apex * ROOT_HALF;
    let feed = build_feed(
        kind,
        lam,
        &FeedFrame { origin: [cx, 0.0, cx], bore: [-ROOT_HALF, 0.0, -ROOT_HALF], pol: [0.0, 1.0, 0.0] },
        Some(driven),
    );
    let plate = |z_axis: bool| -> Vec<Vec3> {
        if z_axis {
            vec![[0.0, -side / 2.0, 0.0], [side, -side / 2.0, 0.0], [side, side / 2.0, 0.0], [0.0, side / 2.0, 0.0]]
        } else {
            vec![[0.0, -side / 2.0, 0.0], [0.0, -side / 2.0, side], [0.0, side / 2.0, side], [0.0, side / 2.0, 0.0]]
        }
    };
    let mut rows = vec![row("feed", kind.label())];
    if kind == FeedKind::Dipole {
        rows.push(row("**driven** dipole, tip to tip", c.fmt(driven)));
    }
    rows.extend([
        row("**apex** to driven element", c.fmt(apex)),
        row("**plate** side length, each", c.fmt(side)),
        row("plate height along the apex", c.fmt(side)),
        row("included angle", "90°"),
        total(
            "total wire needed",
            if kind == FeedKind::Dipole { c.fmt(driven) } else { "see feed".into() },
        ),
    ]);
    let mut geo = WireGeometry::new(feed.lines, feed.feed);
    geo.mirrors = vec![ImagePlane { axis: 0, at: 0.0 }, ImagePlane { axis: 2, at: 0.0 }];
    geo.blocked = Some(Arc::new(|d: Vec3| d[0] < 0.0 || d[2] < 0.0));
    Output {
        spec: "~12 dBi · exact corner images · linear".into(),
        rows,
        scene: Scene {
            wires: feed.wires,
            polys: vec![
                Poly { p: plate(true), fill: Some(hexa(0xc9d1d6, 0.14)), stroke: Some(SILVER) },
                Poly { p: plate(false), fill: Some(hexa(0xc9d1d6, 0.10)), stroke: Some(SILVER) },
            ],
            feed: Some(feed.feed),
            beam_vec: Some([ROOT_HALF, 0.0, ROOT_HALF]),
            pol: feed.pol.into(),
            ..Default::default()
        },
        solve: Geometry::Wire(geo),
        diagram: diagram(apex, side, driven),
        feed: inline(
            "driven element, one half",
            "driven element, other half",
            Some("Keep the dipole parallel to the apex line. Rotate it and the images stop reinforcing."),
        ),
        notes: format!(
            "{}\n\n**Two flat plates and one dipole, for about 12 dBi.** Nothing is resonant except \
             the driven element, so there is no tuning to get wrong: the plates just have to be \
             flat, meet at 90°, and be big enough. Sheet metal, wire mesh with holes under λ/10, or \
             even an oven shelf all work, because the reflector does not care about the \
             difference.\n\n**Apex spacing is the one number that matters.** Around 0.5 λ from apex \
             to dipole is the usual choice: closer and the radiation resistance collapses toward a \
             few ohms as the images cancel the driven element, further and the pattern grows side \
             lobes. Change it above and watch both the impedance and the gain move.\n\n**The \
             solver treats the corner as infinite**, using the exact three images with signs plus, \
             minus, minus, plus. That is the textbook construction and it is right in front of the \
             corner, where you care. It flatters the front-to-back ratio, since real plates of a \
             wavelength or so leak round the edges. Treat gain as an upper bound and the forward \
             lobe shape as accurate.\n\n**Plate size is therefore a construction note, not a solved \
             dimension**, and the table says so rather than pretending otherwise: changing it moves \
             the drawing and nothing else. Make each plate about a wavelength square, extending \
             roughly 0.6 λ beyond the dipole along the apex, and the infinite assumption is close \
             enough to be useful.",
            feed.note
        ),
        cut: Some(CutFile {
            loops: vec![
                vec![[0.0, 0.0, 0.0], [side, 0.0, 0.0], [side, side, 0.0], [0.0, side, 0.0]],
                vec![[side * 1.1, 0.0, 0.0], [side * 2.1, 0.0, 0.0], [side * 2.1, side, 0.0], [side * 1.1, side, 0.0]],
            ],
            circles: vec![],
            note: format!(
                "Two plates, {} square. Join them along one edge at exactly 90 degrees: the angle \
                 matters more than the size, since it is what puts the images where the solver \
                 assumes they are.",
                c.fmt(side)
            ),
        }),
    }
}
