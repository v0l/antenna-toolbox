use crate::draw::{Anchor, DIM, Drawing, GOLD, GREEN, GREY, SILVER, hex};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::units::C;
use std::f64::consts::PI;

pub static DISCONE: Design = Design {
    id: "discone",
    name: "Discone",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2 dBi",
    controls: &[ControlId::Spokes],
    polarisation: "**Vertical, omnidirectional, and almost frequency-independent.** The pattern \
                   and the impedance hold across a decade of frequency, which no other antenna here \
                   comes close to. That is the whole point of it: one antenna for a scanner or a \
                   spectrum survey instead of a drawer full of resonant ones.",
    compute,
};

fn diagram(s: f64, disc_r: f64, gap: f64, half: f64, n: usize) -> Drawing {
    let w = 640.0;
    let cx = w / 2.0;
    let top = 60.0;
    let bw = s * half.sin();
    let bh = s * half.cos();
    let sc = (250.0 / disc_r.max(bw)).min(240.0 / bh);
    let yd = top;
    let ya = top + (gap * sc).max(10.0);
    let yb = ya + bh * sc;
    let h = yb + 76.0;
    let mut g = Drawing::new(w, h);
    g.line(cx - disc_r * sc, yd, cx + disc_r * sc, yd, GOLD, 3.5);
    g.line(cx, ya, cx - bw * sc, yb, SILVER, 3.0);
    g.line(cx, ya, cx + bw * sc, yb, SILVER, 3.0);
    g.line(cx, ya, cx - bw * sc * 0.45, yb, hex(0x6b757b), 2.0);
    g.line(cx, ya, cx + bw * sc * 0.45, yb, hex(0x6b757b), 2.0);
    g.line(cx - disc_r * sc * 0.5, yd, cx + disc_r * sc * 0.5, yd, hex(0xb08020), 2.0);
    g.dot(cx, yd, 3.2, GOLD);
    g.dot(cx, ya, 3.2, GREY);
    g.label(cx + disc_r * sc + 10.0, yd + 4.0, format!("disc, {n} spokes, on the pin"), GOLD, 12.0, Anchor::Start);
    g.label(cx + bw * sc + 10.0, yb + 4.0, format!("cone, {n} spokes, on the shield"), SILVER, 12.0, Anchor::Start);
    g.dim(cx - disc_r * sc - 24.0, yd, cx - disc_r * sc - 24.0, ya, "gap", -6.0, 4.0, Anchor::End);
    g.mono(cx - bw * sc * 0.55, (ya + yb) / 2.0, "slant", DIM, 12.0, Anchor::End);
    g.label(cx, h - 14.0, "side view · equal in all horizontal directions", GREEN, 12.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let n = c.ctl(ControlId::Spokes) as usize;
    let s = c.P("cone slant", 0.25 * lam);
    let disc_r = c.P("disc radius", 0.35 * 0.25 * lam);
    let gap = c.P("gap", 0.008 * lam);
    let half = 30f64.to_radians();
    let f = C / lam;
    let mut wires = Vec::new();
    let mut lines = vec![vec![[0.0, -gap / 2.0, 0.0], [0.0, gap / 2.0, 0.0]]];
    for i in 0..n {
        let th = i as f64 * 2.0 * PI / n as f64;
        let disc = vec![[0.0, gap / 2.0, 0.0], [disc_r * th.cos(), gap / 2.0, disc_r * th.sin()]];
        let cone = vec![
            [0.0, -gap / 2.0, 0.0],
            [s * half.sin() * th.cos(), -gap / 2.0 - s * half.cos(), s * half.sin() * th.sin()],
        ];
        wires.push(wire(disc.clone(), GOLD, 2.6));
        wires.push(wire(cone.clone(), SILVER, 2.6));
        lines.push(disc);
        lines.push(cone);
    }
    Output {
        spec: "~2 dBi · vertical · 50 Ω over a decade".into(),
        rows: vec![
            row("**cone spoke** slant length", c.fmt(s)),
            row("**disc spoke** length, radius", c.fmt(disc_r)),
            row("disc diameter", c.fmt(2.0 * disc_r)),
            row("cone half angle from vertical", "30°"),
            row("cone base diameter", c.fmt(2.0 * s * half.sin())),
            row("cone vertical height", c.fmt(s * half.cos())),
            row("**gap** disc to cone apex", c.fmt(gap)),
            row("spokes, each section", n.to_string()),
            row("usable range", format!("{:.2} to {:.2} GHz", f / 1000.0, 4.0 * f / 1000.0)),
            total("total wire needed", c.fmt(n as f64 * (s + disc_r))),
        ],
        scene: Scene {
            wires,
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(-s * half.cos() / 2.0),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(lines, [0.0; 3])),
        diagram: diagram(s, disc_r, gap, half, n),
        feed: vertical(
            "disc spokes, on the centre pin",
            "cone spokes, on the shield",
            Some("Solder the disc spokes into a ring at the centre so they share one joint on the pin."),
        ),
        notes: "**One antenna instead of a drawer full of them.** The disc sits on the centre pin, \
                the cone hangs off the shield, and because there is no single resonant length the \
                whole thing stays near 50 Ω from the design frequency up to roughly ten times it. \
                Below the design frequency it falls off a cliff, so pick the lowest frequency you \
                care about and build for that.\n\n**Wire spokes are as good as sheet metal** as \
                long as you use enough of them. Eight per section is the usual minimum, sixteen is \
                noticeably better at the top of the range, where the gaps between spokes start to \
                look large compared to a wavelength.\n\nThe gap between disc and cone apex is small \
                and it matters: too wide and the high end suffers. The cone half angle is not \
                critical anywhere between 25° and 35°. Gain is close to a dipole, which is the \
                trade you make for the bandwidth."
            .into(),
        cut: None,
    }
}
