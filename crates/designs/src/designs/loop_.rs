use crate::draw::{Anchor, Drawing, GOLD, MUTED, Stroke};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;
use std::f64::consts::PI;

pub static LOOP: Design = Design {
    id: "loop",
    name: "Wire loop",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "3 dBi",
    controls: &[ControlId::LoopCirc],
    polarisation: "**Linear, parallel to the feed point.** A loop is polarised along the tangent \
                   at the gap, so a loop fed at the bottom is horizontally polarised even though \
                   the loop stands upright. Move the feed a quarter of the way round and the \
                   polarisation turns with it, which is the easiest way to match a loop to \
                   whatever you are talking to.",
    compute,
};

fn diagram(dia: f64, circ: f64, fmt: &dyn Fn(f64) -> String) -> Drawing {
    let (w, h) = (640.0, 330.0);
    let cx = w / 2.0;
    let cy = h / 2.0 - 10.0;
    let r = 110.0;
    let mut g = Drawing::new(w, h);
    let gap = 7.0 / r;
    g.arc(cx, cy, r, r, PI / 2.0 + gap, PI / 2.0 + 2.0 * PI - gap, Stroke::new(GOLD, 3.5));
    g.dot(cx, cy + r, 2.8, GOLD);
    g.dim(cx - r, cy, cx + r, cy, "diameter", 0.0, -8.0, Anchor::Middle);
    g.label(cx, cy + r + 26.0, "feed gap", GOLD, 11.0, Anchor::Middle);
    g.label(cx, 30.0, format!("{} of wire around the circle", fmt(circ)), MUTED, 12.0, Anchor::Middle);
    g.label(cx, h - 12.0, format!("{} across", fmt(dia)), MUTED, 12.0, Anchor::Middle);
    g.beam_label(w - 80.0, cy, Some("beam, both ways"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let circ_lam = c.ctl(ControlId::LoopCirc);
    let circ = c.P("circumference", circ_lam * lam);
    let dia = circ / PI;
    let r = dia / 2.0;
    let gap = (lam / 120.0).min(circ / 60.0).max(2.0 * c.wire_dia);
    let half_gap = gap / circ * PI;
    let steps = 72;
    let pts: Vec<Vec3> = (0..=steps)
        .map(|i| {
            let a = -PI / 2.0 + half_gap + i as f64 / steps as f64 * (2.0 * PI - 2.0 * half_gap);
            [r * a.cos(), r * a.sin(), 0.0]
        })
        .collect();
    let ends = vec![pts[pts.len() - 1], pts[0]];
    let feed = [0.0, -r * half_gap.cos(), 0.0];
    let full = circ_lam >= 0.75;
    let small = circ_lam <= 0.35;
    Output {
        spec: if full {
            "~3.4 dBi · bidirectional · ~135 Ω at 1 λ"
        } else if small {
            "small loop · tiny radiation resistance"
        } else {
            "between the two regimes"
        }
        .into(),
        rows: vec![
            row("**circumference** of the loop", c.fmt(circ)),
            row("circumference in wavelengths", format!("{circ_lam:.2} λ")),
            row("diameter across", c.fmt(dia)),
            row("feed gap at the bottom", c.fmt(gap)),
            total("total wire needed", c.fmt(circ)),
        ],
        scene: Scene {
            wires: vec![wire(pts.clone(), GOLD, 3.5)],
            feed: Some([0.0, -r, 0.0]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pol: "polarisation: linear, along the tangent at the feed gap".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(vec![pts, ends], feed)),
        diagram: diagram(dia, circ, c.fmt),
        feed: vertical(
            "one end of the wire, on the centre pin",
            "the other end, on the shield",
            Some("Keep the gap small and the two ends level, or the loop stops being symmetric."),
        ),
        notes: "**One wavelength of wire bent into a circle.** At a full wavelength around, a \
                loop is a proper antenna: about 3.4 dBi, roughly 135 Ω, and a clean two-sided \
                pattern broadside to the loop, with nulls in the plane of the wire. That is a \
                decibel better than a dipole, for the same reason a quad element beats one, and it \
                is less bothered by nearby metal.\n\n**Shrink it and the physics turns against \
                you.** Radiation resistance of a small loop falls as the fourth power of \
                circumference, so halving the loop cuts it by sixteen. Below about a third of a \
                wavelength the model still reports a tidy impedance, but it assumes perfect wire: \
                a real small loop has its radiation resistance buried under the loss resistance of \
                the copper, and the efficiency that follows is measured in percent. That is why a \
                transmitting magnetic loop is built from fat tube rather than wire, and why this \
                calculator will flatter one.\n\n**It is not resonant at one wavelength.** Reactance \
                crosses zero nearer 1.1 λ around, where resistance has climbed past 200 Ω, so a \
                loop cut to exactly 1 λ is capacitive. Neither is 50 Ω: a quarter-wave of 75 Ω coax \
                makes a decent transformer.\n\nPull the circumference control up towards a \
                wavelength and watch the resistance climb out of the noise. The jump is steep, and \
                it is the single reason full-wave loops are common and small ones are specialist."
            .into(),
        cut: None,
    }
}
