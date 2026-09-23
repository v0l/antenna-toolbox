use crate::draw::{Anchor, GOLD, GREEN, SILVER};
use crate::feed_detail::vertical;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, Load, WireGeometry};
use std::f64::consts::PI;

pub static FIVE_EIGHTHS: Design = Design {
    id: "fiveeighths",
    name: "5/8-wave vertical",
    group: Group::Omni,
    build: Build::Wire,
    gain: "8 dBi",
    controls: &[],
    polarisation: "**Vertical, omnidirectional in azimuth.** Flatter than the quarter wave: the \
                   longer radiator pushes more of the power down toward the horizon, which is why \
                   it is the classic mobile and base whip for VHF.",
    compute,
};

pub const COIL_OHMS: f64 = 224.0;

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let len = c.P("radiator", 0.635 * lam);
    let f_hz = 299_792_458.0 / (lam / 1000.0);
    let l_nh = COIL_OHMS / (2.0 * PI * f_hz) * 1e9;
    let mut geo = WireGeometry::new(vec![vec![[0.0; 3], [0.0, 0.0, len]]], [0.0; 3]).ground(0.0);
    geo.loads.push(([0.0; 3], Load::Series { r: 0.0, l: l_nh * 1e-9, c: 0.0 }));
    let coil_h = 0.03 * lam;
    Output {
        spec: format!(
            "~8 dBi over a large plate · vertical · 50 Ω through a {l_nh:.0} nH base coil"
        ),
        rows: vec![
            row("**radiator**, above the coil", c.fmt(len)),
            row("**base coil**, in series with the radiator", format!("{l_nh:.0} nH")),
            row("coil reactance at the design frequency", format!("+j{COIL_OHMS:.0} Ω")),
            row("ground plane, minimum radius", c.fmt(0.25 * lam)),
            total("total wire in the radiator", c.fmt(len)),
        ],
        scene: Scene {
            wires: vec![wire(vec![[0.0; 3], [0.0, 0.0, len]], GOLD, 3.5)],
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(len / 2.0),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(geo),
        diagram: Sketch::new()
            .ground(0.0)
            .wire(&[(0.0, coil_h), (0.0, len)], GOLD, 3.5)
            .wire(
                &(0..=40)
                    .map(|i| {
                        let t = i as f64 / 40.0;
                        (0.012 * lam * (t * 10.0 * PI).sin(), t * coil_h)
                    })
                    .collect::<Vec<_>>(),
                SILVER,
                2.0,
            )
            .note(
                (0.02 * lam, coil_h / 2.0),
                format!("base coil, {l_nh:.0} nH"),
                SILVER,
                Anchor::Start,
            )
            .dim((-0.05 * len, coil_h), (-0.05 * len, len), "radiator", (-8.0, 4.0), Anchor::End)
            .caption("equal in all horizontal directions, low and flat", GREEN)
            .tall(360.0)
            .render(),
        feed: vertical(
            "bottom of the coil, on the centre pin",
            "plate or roof, on the shield",
            Some("The coil sits between the connector and the radiator, not across it."),
        ),
        notes: "**The longest single radiator that still sends its power to the horizon.** Past \
                about 0.64 λ the current along the wire reverses and a lobe breaks away upward, \
                so 5/8 λ is the sweet spot. Over a large plate it gives about 3 dB more than a \
                quarter wave at the horizon.\n\n**The coil is the match.** A 5/8 λ wire on its \
                own is around 50 Ω resistive but strongly capacitive. A series coil at the base \
                cancels that. The coil here is solved as an ideal inductor; wind it, then \
                spread or squeeze the turns while watching the analyser, since a few nH either \
                way moves the match a long way. Commercial whips often use a tapped coil to \
                ground instead, which does the same job and also puts the radiator at DC \
                ground.\n\n**The plate matters more than for a quarter wave.** A 5/8 on a small \
                ground plane or a handheld loses most of its advantage, and the radiation angle \
                creeps up. Use it on a car roof or with radials at least a quarter wave long."
            .into(),
        cut: None,
    }
}
