use super::patch::{board, m, merge, outline, rect};
use crate::draw::{Anchor, Drawing, GOLD, GREEN, GREY, SILVER, Stroke, hexa};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total};
use antenna_solver::fdtd::{Aabb, Dielectric, Model, Port};
use antenna_solver::geometry::Geometry;

pub static IFA: Design = Design {
    id: "ifa",
    name: "Printed inverted-F",
    group: Group::Omni,
    build: Build::Pcb,
    gain: "1 dBi",
    controls: &[ControlId::Substrate, ControlId::Thickness],
    polarisation: "**Mostly along the arm, filled in all round.** The arm and the edge of the \
                   ground plane both carry current, so the pattern is a lumpy doughnut rather \
                   than a clean dipole. Good enough for a sensor that could end up any way up, \
                   which is what it is for.",
    compute,
};

struct Shape {
    arm: f64,
    height: f64,
    feed: f64,
    trace: f64,
    ground: f64,
    width: f64,
    clear: f64,
}

fn diagram(s: &Shape) -> Drawing {
    let (dw, dh) = (640.0, 440.0);
    let total_h = s.ground + s.clear;
    let sc = (380.0 / s.width).min(340.0 / total_h);
    let x0 = 320.0 - s.width / 2.0 * sc;
    let y0 = 40.0;
    let yg = y0 + s.clear * sc;
    let mut g = Drawing::new(dw, dh);
    g.rect(
        x0,
        y0,
        s.width * sc,
        total_h * sc,
        Some(hexa(0x2f6b3a, 0.25)),
        Some(Stroke::new(GREY, 1.0)),
    );
    g.rect(
        x0,
        yg,
        s.width * sc,
        s.ground * sc,
        Some(hexa(0xc9d1d6, 0.22)),
        Some(Stroke::new(SILVER, 2.0)),
    );
    let xs = x0 + 3.0 * s.trace * sc;
    let arm_y = yg - s.height * sc;
    g.line(xs, yg, xs, arm_y, GOLD, 3.0);
    g.line(xs, arm_y, xs + s.arm * sc, arm_y, GOLD, 3.0);
    let xf = xs + s.feed * sc;
    g.line(xf, yg - 6.0, xf, arm_y, GOLD, 3.0);
    g.dot(xf, yg - 3.0, 3.0, GOLD);
    g.label(xf + 6.0, yg - 10.0, "feed", GOLD, 11.0, Anchor::Start);
    g.label(xs - 6.0, (yg + arm_y) / 2.0, "short", GREY, 11.0, Anchor::End);
    g.dim(xs, arm_y - 16.0, xs + s.arm * sc, arm_y - 16.0, "arm", 0.0, -6.0, Anchor::Middle);
    g.dim(
        xs + s.arm * sc + 16.0,
        yg,
        xs + s.arm * sc + 16.0,
        arm_y,
        "height",
        8.0,
        4.0,
        Anchor::Start,
    );
    g.label(320.0, yg + s.ground * sc / 2.0, "ground, both layers", GREY, 12.0, Anchor::Middle);
    g.label(320.0, dh - 12.0, "roughly the same in every direction", GREEN, 12.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let b = board(c);
    let ee = 1.0 + (b.eps - 1.0) * 0.17 * (b.h / 1.6).powf(0.3);
    let quarter = lam / 4.0 / ee.sqrt();
    let s = Shape {
        height: c.P("arm height", 0.035 * lam),
        arm: c.P("arm length", quarter - 0.035 * lam),
        feed: c.P("feed from short", 0.012 * lam),
        trace: c.P("trace width", (lam / 300.0).max(0.3)),
        ground: c.P("ground length", 0.25 * lam),
        width: c.P("board width", quarter + 0.06 * lam),
        clear: 0.0,
    };
    let s = Shape { clear: s.height + 3.0 * s.trace, ..s };
    let h = b.h;
    let (xl, xr) = (-s.width / 2.0, s.width / 2.0);
    let yg = 0.0;
    let xs = xl + 3.0 * s.trace;
    let xf = xs + s.feed;
    let t = s.trace;
    let gap = t;
    let arm_y = yg + s.height;
    let metal = [
        ([xl, yg - s.ground], [xr, yg]),
        ([xs - t / 2.0, yg], [xs + t / 2.0, arm_y + t / 2.0]),
        ([xs - t / 2.0, arm_y - t / 2.0], [xs + s.arm, arm_y + t / 2.0]),
        ([xf - t / 2.0, yg + gap], [xf + t / 2.0, arm_y]),
    ];
    let model = Model {
        metal: metal
            .iter()
            .map(|(lo, hi)| Aabb::new([m(lo[0]), m(lo[1]), m(h)], [m(hi[0]), m(hi[1]), m(h)]))
            .chain(std::iter::once(Aabb::new([m(xl), m(yg - s.ground), 0.0], [m(xr), m(yg), 0.0])))
            .collect(),
        dielectrics: vec![Dielectric {
            bounds: Aabb::new([m(xl), m(yg - s.ground), 0.0], [m(xr), m(yg + s.clear), m(h)]),
            eps_r: b.eps,
            tan_d: b.tan,
        }],
        port: Some(Port { a: [m(xf), m(yg), m(h)], b: [m(xf), m(yg + gap), m(h)], r: 50.0 }),
        f0: 299_792_458.0 / m(lam),
        span: 0.1,
        fine: 0.0,
        ground_plane_z: None,
    };
    let parts: Vec<_> =
        metal[1..].iter().map(|(lo, hi)| rect([lo[0], lo[1], h], [hi[0], hi[1], h])).collect();
    Output {
        spec: format!("~1 dBi · printed on {} · solved with FDTD", b.name),
        rows: vec![
            row("**arm length**, from the short", c.fmt(s.arm)),
            row("**arm height** above the ground edge", c.fmt(s.height)),
            row("**feed**, from the short", c.fmt(s.feed)),
            row("trace width", c.fmt(s.trace)),
            row("ground length", c.fmt(s.ground)),
            row("board width", c.fmt(s.width)),
            row(format!("board, {} εr {:.2}", b.name, b.eps), c.fmt(h)),
            total("keep-out above the ground", c.fmt(s.clear)),
        ],
        scene: Scene {
            mesh: Some(merge(&parts)),
            polys: vec![
                outline([xl, yg - s.ground], [xr, yg], h),
                outline([xl, yg - s.ground], [xr, yg + s.clear], 0.0),
            ],
            feed: Some([xf, yg + gap / 2.0, h]),
            pattern_origin: Some([0.0, 0.0, h]),
            up: Some([0.0, 1.0, 0.0]),
            omni: true,
            omni_y: Some(0.0),
            pol: "polarisation: mostly along the arm".into(),
            ..Default::default()
        },
        solve: Geometry::Volume(model),
        diagram: diagram(&s),
        feed: inline(
            "feed trace, on the pin",
            "ground edge, on the shield",
            Some(
                "The feed gap is where a 50 Ω coplanar line or a U.FL connector lands on the \
                 board: pin to the feed trace, shield to the ground pour either side of it.",
            ),
        ),
        notes: "**A quarter-wave monopole folded over and fed off-centre.** The arm runs along \
                the edge of the board a short height above the ground pour; one end is shorted \
                straight down to the ground and the feed taps in a little way along from the \
                short. That tap is a transformer: moving it away from the short raises the \
                resistance, toward it lowers it. The arm length sets the frequency.\n\n**The \
                ground pour is the other half.** Current runs along its edge as much as along the \
                arm, so the board length matters and the antenna detunes when the board is \
                shortened. Keep the pour solid right up to the keep-out and keep components out \
                from under the arm.\n\n**Solved with FDTD on the board itself**, substrate loss \
                included, so the frequency shift from the FR-4 and the power it absorbs are \
                both in the result."
            .into(),
        cut: None,
    }
}
