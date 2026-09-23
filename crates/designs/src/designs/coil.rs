use crate::draw::{Anchor, Drawing, GOLD, MUTED, SILVER};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;
use std::f64::consts::PI;

pub static COIL: Design = Design {
    id: "coil",
    name: "Helical whip",
    group: Group::Omni,
    build: Build::Wire,
    gain: "0 dBi",
    controls: &[ControlId::CoilTurns],
    polarisation: "**Vertical, omnidirectional in azimuth.** A normal-mode helix radiates like the \
                   short vertical it is, not like the axial helix in the beams row: the coil is \
                   there to make a short wire resonate, not to make a beam. Same doughnut as a \
                   monopole, a little squashed.",
    compute,
};

fn diagram(height: f64, dia: f64, turns: f64, fmt: &dyn Fn(f64) -> String) -> Drawing {
    let (w, h) = (640.0, 340.0);
    let cx = w / 2.0;
    let base = h - 60.0;
    let hh = 210.0;
    let rr = (hh * dia / height / 2.0).max(10.0);
    let steps = (turns * 24.0).round() as usize;
    let pts: Vec<(f64, f64)> = (0..=steps)
        .map(|i| {
            let t = i as f64 / 24.0;
            (cx + rr * (t * 2.0 * PI).sin(), base - t / turns * hh)
        })
        .collect();
    let mut g = Drawing::new(w, h);
    g.line(cx - 130.0, base, cx + 130.0, base, SILVER, 2.5);
    g.label(cx + 136.0, base + 4.0, "ground plane", SILVER, 11.0, Anchor::Start);
    g.polyline(&pts, GOLD, 2.6);
    g.dot(cx, base, 2.8, GOLD);
    g.dim(cx - 90.0, base, cx - 90.0, base - hh, "height", -8.0, 4.0, Anchor::End);
    g.dim(
        cx - rr,
        base - hh - 22.0,
        cx + rr,
        base - hh - 22.0,
        "coil ø",
        0.0,
        -8.0,
        Anchor::Middle,
    );
    g.label(
        cx + 120.0,
        base - hh / 2.0,
        format!("{turns} turns, {} tall", fmt(height)),
        MUTED,
        12.0,
        Anchor::Start,
    );
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let turns = c.ctl(ControlId::CoilTurns);
    let height = c.P("height", 0.05 * lam);
    let dia = c.P("coil diameter", 0.02 * lam);
    let r = dia / 2.0;
    let pitch = height / turns;
    let wire_per_turn = (PI * dia).hypot(pitch);
    let gz = -(lam / 200.0).max(2.0 * c.wire_dia);
    let steps = ((turns * 24.0).round() as usize).max(16);
    let pts: Vec<Vec3> = (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64 * turns;
            [r * (t * 2.0 * PI).cos(), r * (t * 2.0 * PI).sin(), t / turns * height]
        })
        .collect();
    Output {
        spec: "short vertical · 4.8 dBi ideal, far less once built".into(),
        rows: vec![
            row("**height** of the coil", c.fmt(height)),
            row("**coil diameter**", c.fmt(dia)),
            row("turns", format!("{turns}")),
            row("pitch, one turn to the next", c.fmt(pitch)),
            row("height in wavelengths", format!("{:.3} λ", height / lam)),
            row("ground plane diameter, minimum", c.fmt(0.5 * lam)),
            total(format!("total wire, N × {}", c.fmt(wire_per_turn)), c.fmt(turns * wire_per_turn)),
        ],
        scene: Scene {
            wires: vec![wire(pts.clone(), GOLD, 2.6), wire(vec![[r, 0.0, gz], [r, 0.0, 0.0]], GOLD, 3.5)],
            feed: Some([r, 0.0, gz / 2.0]),
            omni: true,
            omni_y: Some(height / 2.0),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(
            WireGeometry::new(vec![pts.clone(), vec![[r, 0.0, gz], pts[0]]], [r, 0.0, gz / 2.0]).ground(gz),
        ),
        diagram: diagram(height, dia, turns, c.fmt),
        feed: vertical(
            "bottom of the coil, on the centre pin",
            "ground plane, on the shield",
            Some(
                "The coil must start right at the connector. Any straight wire below it is a \
                 different antenna and shifts resonance.",
            ),
        ),
        notes: "**The rubber duck, laid bare.** Winding a quarter wavelength of wire into a coil \
                makes it resonate at a height far below a quarter wave, because the coil supplies \
                the inductance the missing length would have. Nothing else about it improves: the \
                current is squeezed into a shorter span, so it radiates less well than the straight \
                whip it replaces.\n\n**The gain figure here is a ceiling you will not reach.** The \
                solver reports about 4.8 dBi, which is right for the shape: a short vertical over \
                perfect ground has a directivity of 4.77 dBi no matter how short it is, because \
                squashing it changes how well it is driven, not where it points. What the model \
                leaves out is loss. At the default size the radiation resistance is only a few \
                ohms, so the coil's own resistance sits alongside it and eats most of the power as \
                heat. That is the whole story of the rubber duck: the pattern is fine, the \
                efficiency is not, and a real one lands nearer 0 dBi.\n\n**Expect an awkward \
                feed.** Radiation resistance drops with the square of height, so a coil this short \
                lands well under 50 Ω with reactance swinging fast either side of resonance. Add or \
                remove a turn and it moves a long way. Build it slightly long, then trim the top \
                turn while watching the analyser, and accept that the bandwidth is narrow.\n\n\
                Turns, height and diameter all trade against each other: more turns in the same \
                height lowers resonance, and so does a fatter coil. Only the height sets how well \
                it radiates, which is the whole bargain. Use the tune button to scale it to \
                resonance, then compare its gain against the plain ground plane or vertical dipole \
                to see what the shortening costs."
            .into(),
        cut: None,
    }
}
