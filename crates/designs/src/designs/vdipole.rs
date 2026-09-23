use crate::draw::{Anchor, DIM, Drawing, GOLD, GREEN, GREY, MUTED, Stroke};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use std::f64::consts::PI;

pub static VDIPOLE: Design = Design {
    id: "vdip",
    name: "V-dipole",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2 dBi",
    controls: &[ControlId::Angle],
    polarisation: "**Linear, but pointed at the sky.** A dipole bent into a V, apex down, so the \
                   pattern is a broad dome overhead instead of a doughnut around the horizon. It \
                   hears a polar-orbiting satellite from horizon to horizon without a rotator, \
                   which is the whole reason it exists. Against a circularly polarised downlink you \
                   lose the usual 3 dB, and nobody minds, because the alternative is a turnstile or \
                   a QFH you have to build properly.",
    compute,
};

fn diagram(arm: f64, inc: f64) -> Drawing {
    let w = 640.0;
    let cx = w / 2.0;
    let half = (inc / 2.0).to_radians();
    let sc = (250.0 / (half.sin() * arm)).min(230.0 / (half.cos() * arm).max(1.0));
    let dx = half.sin() * arm * sc;
    let dy = half.cos() * arm * sc;
    let cy = 60.0 + dy;
    let h = cy + 80.0;
    let mut g = Drawing::new(w, h);
    g.line(cx - 4.0, cy, cx - dx, cy - dy, GOLD, 3.5);
    g.line(cx + 4.0, cy, cx + dx, cy - dy, GOLD, 3.5);
    g.dot(cx - 4.0, cy, 2.8, GOLD);
    g.dot(cx + 4.0, cy, 2.8, GREY);
    g.arc(cx, cy, 34.0, 34.0, -PI / 2.0 - half, -PI / 2.0 + half, Stroke::new(DIM, 1.0));
    g.mono(cx, cy - 44.0, format!("{inc}°"), DIM, 12.0, Anchor::Middle);
    g.label(cx - dx - 8.0, cy - dy - 8.0, "arm", GOLD, 12.0, Anchor::End);
    g.line(cx, cy + 14.0, cx, cy + 40.0, MUTED, 4.0);
    g.label(cx + 10.0, cy + 34.0, "coax, straight down", MUTED, 12.0, Anchor::Start);
    g.stroke_path(
        vec![
            [(cx - dx) as f32, (cy - dy - 26.0) as f32],
            [(cx + dx) as f32, (cy - dy - 26.0) as f32],
        ],
        false,
        Stroke::dashed(GREEN, 1.0, 4.0, 4.0),
    );
    g.label(cx, cy - dy - 32.0, "↑ sky ↑", GREEN, 12.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let inc = c.ctl(ControlId::Angle);
    let arm = c.P("arm", 0.24 * lam);
    let half = (inc / 2.0).to_radians();
    let e = lam / 120.0;
    let tip = [half.sin() * arm, half.cos() * arm];
    Output {
        spec: "solving…".into(),
        rows: vec![
            row("**arm** length, each of 2", c.fmt(arm)),
            row("tip to tip, straight line", c.fmt(2.0 * half.sin() * arm)),
            row("included angle", format!("{inc}°")),
            row("arm elevation above horizontal", format!("{:.0}°", 90.0 - inc / 2.0)),
            row("height of the tips above the feed", c.fmt(tip[1])),
            total("total wire needed", c.fmt(2.0 * arm)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[-e, 0.0, 0.0], [-tip[0], tip[1], 0.0]], GOLD, 3.5),
                wire(vec![[e, 0.0, 0.0], [tip[0], tip[1], 0.0]], GOLD, 3.5),
            ],
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 1.0, 0.0]),
            pattern_origin: Some([0.0, tip[1] / 3.0, 0.0]),
            pol: "polarisation: linear, broad lobe overhead".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![
                vec![[-e, 0.0, 0.0], [-tip[0], tip[1], 0.0]],
                vec![[e, 0.0, 0.0], [tip[0], tip[1], 0.0]],
                vec![[-e, 0.0, 0.0], [e, 0.0, 0.0]],
            ],
            [0.0; 3],
        )),
        diagram: diagram(arm, inc),
        feed: vertical(
            "one arm, on the centre pin",
            "other arm, on the shield",
            Some("Bend the arms after soldering, so both roots stay at the connector."),
        ),
        notes: "**The antenna to build if you want weather satellite pictures tonight.** Two \
                quarter-wave arms, one on the pin and one on the shield, spread to 120° with the \
                point of the V facing down. Cut for 137.5 MHz that is about 53 cm per arm. Mount it \
                a metre or two up with the plane of the V running north to south, so a polar orbit \
                crosses along it.\n\n**120° is the number that matters.** A straight dipole (180°) \
                has a null straight up where the satellite is loudest, and a tight V behaves like \
                two wires fighting each other. At 120° the pattern flattens into a wide dome and \
                the impedance lands near 50 Ω without help. Change the angle above and watch the \
                solved SWR move.\n\nHorizontal polarisation into a circularly polarised downlink \
                costs a flat 3 dB, which for NOAA APT and Meteor LRPT is irrelevant: they are loud, \
                and a clean 3 dB loss beats a badly built QFH. Choke the coax at the feed, as with \
                any balanced antenna."
            .into(),
        cut: None,
    }
}
