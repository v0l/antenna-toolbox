use crate::draw::{Anchor, DIM, Drawing, GOLD, GREEN, GREY, SILVER, Stroke};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use std::f64::consts::PI;

pub static GROUNDPLANE: Design = Design {
    id: "gp",
    name: "Ground plane",
    group: Group::Omni,
    build: Build::Wire,
    gain: "1 dBi",
    controls: &[ControlId::Droop],
    polarisation: "**Vertical, and omnidirectional in azimuth.** Equal in every compass \
                   direction, with a deep null straight up and most of the power thrown at a \
                   shallow angle, which is what you want for talking across ground. Turning it on \
                   its side does not give you a horizontal omni, it gives you a mess.",
    compute,
};

fn diagram(rad: f64, rl: f64, dr: f64) -> Drawing {
    let w = 640.0;
    let cy = 150.0;
    let cx = w / 2.0;
    let angle = dr.to_radians();
    let sc = (120.0 / rad).min(230.0 / (rl * angle.cos()));
    let rx = rl * angle.cos() * sc;
    let ry = rl * angle.sin() * sc;
    let h = cy + ry + 80.0;
    let mut g = Drawing::new(w, h);
    g.line(cx, cy, cx, cy - rad * sc, GOLD, 3.5);
    g.line(cx, cy, cx - rx, cy + ry, SILVER, 3.0);
    g.line(cx, cy, cx + rx, cy + ry, SILVER, 3.0);
    g.line(cx, cy, cx - rx * 0.45, cy + ry * 0.72, GREY, 2.4);
    g.line(cx, cy, cx + rx * 0.45, cy + ry * 0.72, GREY, 2.4);
    g.dot(cx, cy, 4.0, GREEN);
    g.label(cx + 14.0, cy + 5.0, "SMA", GREEN, 12.0, Anchor::Start);
    g.dim(cx - 40.0, cy, cx - 40.0, cy - rad * sc, "radiator", -6.0, 0.0, Anchor::End);
    g.label(cx + rx + 8.0, cy + ry + 4.0, "radial (4 off)", SILVER, 12.0, Anchor::Start);
    g.arc(cx, cy, 34.0, 34.0, 0.0, angle, Stroke::new(DIM, 1.0));
    g.mono(cx + 44.0, cy + 26.0, format!("{dr}°"), DIM, 12.0, Anchor::Start);
    g.label(cx, h - 14.0, "equal in all horizontal directions", GREEN, 12.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let dr = c.ctl(ControlId::Droop);
    let rad = c.P("radiator", 0.2375 * lam);
    let rl = c.P("radial", 0.2375 * lam * 1.05);
    let z = 36.0 + (50.0 - 36.0) * (dr / 45.0).min(1.0);
    let a = dr.to_radians();
    let mut wires = vec![wire(vec![[0.0; 3], [0.0, rad, 0.0]], GOLD, 3.5)];
    let g = lam / 120.0;
    let mut lines = vec![vec![[0.0, g, 0.0], [0.0, rad, 0.0]], vec![[0.0, -g, 0.0], [0.0, g, 0.0]]];
    for i in 0..4 {
        let th = i as f64 * PI / 2.0;
        let (x, z) = (rl * a.cos() * th.cos(), rl * a.cos() * th.sin());
        wires.push(wire(vec![[0.0; 3], [x, -rl * a.sin(), z]], SILVER, 3.0));
        lines.push(vec![[0.0, -g, 0.0], [x, -g - rl * a.sin(), z]]);
    }
    Output {
        spec: format!("~1 dBi · vertical · ~{z:.0} Ω"),
        rows: vec![
            row("**radiator**, vertical wire", c.fmt(rad)),
            row("**radials**, 4 off, each", c.fmt(rl)),
            row("droop below horizontal", format!("{dr}°")),
            row("total height above the connector", c.fmt(rad)),
            total("total wire needed", c.fmt(rad + 4.0 * rl)),
        ],
        scene: Scene {
            wires,
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(rad * 0.5),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            pattern_origin: Some([0.0, rad / 2.0, 0.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(lines, [0.0; 3])),
        diagram: diagram(rad, rl, dr),
        feed: vertical(
            "radiator, on the centre pin",
            "all four radials, on the shield",
            Some("Solder the radials to the connector body or its mounting flange, all four together."),
        ),
        notes: "**The default answer for a vertical.** One wire up from the centre pin, four wires \
                out from the connector body. The radials are the other half of the antenna, not a \
                grounding afterthought, which is why the thing does not work properly with only \
                one or two.\n\n**Droop sets the impedance.** Radials flat out give about 36 Ω, a \
                poor match on 50 Ω coax. Bending them down to 45° raises it to almost exactly 50 Ω, \
                and that is the only reason the classic drooping shape exists. Anything between 30° \
                and 45° is fine.\n\nFour radials is the practical minimum. More radials buy a \
                slightly cleaner pattern and a little less feedline current, with diminishing \
                returns past about eight. A sheet of metal or a car roof works too, and behaves \
                like infinite radials."
            .into(),
        cut: None,
    }
}
