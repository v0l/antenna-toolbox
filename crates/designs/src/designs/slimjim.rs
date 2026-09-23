use crate::draw::{Anchor, DIM, Drawing, GOLD, GREEN, GREY, Stroke, hex};
use crate::feed_detail::vertical;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static SLIMJIM: Design = Design {
    id: "slimjim",
    name: "Slim Jim",
    group: Group::Omni,
    build: Build::Wire,
    gain: "3 dBi",
    controls: &[],
    polarisation: "**Vertical, omnidirectional, with a slightly squashed doughnut.** The end-fed \
                   half-wave sits higher up than the feedpoint, and the current distribution pushes \
                   a little more energy toward the horizon than a plain dipole does. Worth roughly \
                   1 dB over a dipole in the directions that matter.",
    compute,
};

fn diagram(a: f64, b: f64, gap: f64, tap: f64, ww: f64, fmt: &dyn Fn(f64) -> String) -> Drawing {
    let (w, h) = (640.0, 600.0);
    let sc = 430.0 / a;
    let top = 64.0;
    let bot = top + a * sc;
    let y_of = |v: f64| bot - v * sc;
    let spread = 54.0;
    let cx = 250.0;
    let xl = cx - spread / 2.0;
    let xr = cx + spread / 2.0;
    let y_tap = y_of(tap);
    let mut g = Drawing::new(w, h);
    g.polyline(&[(xl, bot), (xl, top), (xr, top), (xr, y_of(b + gap))], GOLD, 3.5);
    g.polyline(&[(xr, y_of(b)), (xr, bot), (xl, bot)], GOLD, 3.5);
    g.line(xl, y_tap, xl - 40.0, y_tap, GOLD, 2.0);
    g.line(xr, y_tap, xr + 40.0, y_tap, GREY, 2.0);
    g.line(xl - 40.0, y_tap, xl - 40.0, bot + 38.0, GOLD, 2.0);
    g.line(xr + 40.0, y_tap, xr + 40.0, bot + 38.0, GREY, 2.0);
    g.line(xl - 40.0, bot + 38.0, xr + 40.0, bot + 38.0, hex(0x5b6970), 8.0);
    g.dot(cx, bot + 38.0, 3.0, hex(0x0a0e11));
    g.dot(xl, y_tap, 4.0, GOLD);
    g.dot(xr, y_tap, 4.0, GREY);
    g.label(cx, bot + 66.0, "SMA here, slide both ends together for lowest SWR", GREEN, 13.0, Anchor::Middle);
    g.label(xl - 46.0, y_tap - 9.0, "centre pin", GOLD, 12.0, Anchor::End);
    g.label(xr + 46.0, y_tap - 26.0, "shield", GREY, 12.0, Anchor::Start);
    let yg = (y_of(b) + y_of(b + gap)) / 2.0;
    g.line(xr + 8.0, yg, xr + 96.0, yg - 26.0, DIM, 1.0);
    g.mono(xr + 100.0, yg - 28.0, format!("C gap, {}", fmt(gap)), DIM, 13.0, Anchor::Start);
    g.dim(xl - 96.0, top, xl - 96.0, bot, "A", -8.0, 4.0, Anchor::End);
    g.dim(xr + 96.0, y_of(b), xr + 96.0, bot, "B", 8.0, 4.0, Anchor::Start);
    g.line(xr + 104.0, y_tap, xr + 150.0, y_tap, DIM, 1.0);
    g.mono(xr + 154.0, y_tap + 4.0, format!("D, {}", fmt(tap)), DIM, 13.0, Anchor::Start);
    g.stroke_path(
        vec![[xl as f32, (top - 18.0) as f32], [xr as f32, (top - 18.0) as f32]],
        false,
        Stroke::dashed(DIM, 1.0, 3.0, 3.0),
    );
    g.mono(cx, top - 26.0, "W", DIM, 13.0, Anchor::Middle);
    g.label(
        cx,
        20.0,
        format!("legs drawn far apart for clarity: W is only {}", fmt(ww)),
        hex(0x5b6970),
        11.0,
        Anchor::Middle,
    );
    g.label(cx, h - 14.0, "equal in all horizontal directions", GREEN, 13.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let vf = 0.95;
    let a = c.P("A long leg", 0.75 * lam * vf);
    let b = c.P("B stub", 0.25 * lam * vf);
    let gap = c.P("C gap", 0.02 * lam);
    let tap = c.P("D feed tap", 0.045 * lam);
    let w = c.P("W spacing", 0.02 * lam);
    Output {
        spec: "~3 dBi · vertical · 50 Ω at the tap".into(),
        rows: vec![
            row("**A** long leg, full height", c.fmt(a)),
            row("**B** stub leg, to the gap", c.fmt(b)),
            row("**C** gap in the short leg", c.fmt(gap)),
            row("**D** feed tap above the bottom", c.fmt(tap)),
            row("**W** spacing between legs", c.fmt(w)),
            row("upper section, gap to top", c.fmt(a - b - gap)),
            total("total wire, one loop", c.fmt(2.0 * a + 2.0 * w - gap)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[0.0; 3], [0.0, a, 0.0], [w, a, 0.0], [w, b + gap, 0.0]], GOLD, 3.5),
                wire(vec![[w, b, 0.0], [w, 0.0, 0.0], [0.0; 3]], GOLD, 3.5),
            ],
            feed: Some([w / 2.0, tap, 0.0]),
            omni: true,
            omni_y: Some(a * 0.7),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            pattern_origin: Some([w / 2.0, a * 0.66, 0.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![
                vec![[0.0; 3], [0.0, tap, 0.0], [0.0, a, 0.0], [w, a, 0.0], [w, b + gap, 0.0]],
                vec![[w, b, 0.0], [w, tap, 0.0], [w, 0.0, 0.0], [0.0; 3]],
                vec![[0.0, tap, 0.0], [w, tap, 0.0]],
            ],
            [w / 2.0, tap, 0.0],
        )),
        diagram: diagram(a, b, gap, tap, w, c.fmt),
        feed: vertical(
            "long leg, on the centre pin",
            "stub leg, on the shield",
            Some("Tap both legs at the same height, D above the bottom link."),
        ),
        notes: "**A half-wave that feeds at 50 Ω without a ground plane.** One continuous loop: up \
                the long leg, across the top, down the short leg to the gap, and the quarter-wave \
                stub below the gap does the impedance matching. Nothing else is needed, which is \
                why this is the standard homebrew VHF antenna.\n\n**The tap position is \
                empirical.** D is a starting point, not a specification. Feed across both legs at \
                that height, measure, and slide the connection a few millimetres up or down for the \
                lowest SWR. Everything else stays fixed while you do this.\n\nKeep it away from \
                metal and hold the spacing W constant along the whole length, since a leg that \
                wanders changes the stub impedance. At VHF, wire in a length of plastic conduit is \
                the usual build; above about 500 MHz it becomes small enough to be fiddly and a \
                ground plane is easier."
            .into(),
        cut: None,
    }
}
