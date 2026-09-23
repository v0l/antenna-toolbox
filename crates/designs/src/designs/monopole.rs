use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::vertical;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static MONOPOLE: Design = Design {
    id: "monopole",
    name: "Quarter-wave monopole",
    group: Group::Omni,
    build: Build::Wire,
    gain: "5.1 dBi",
    controls: &[],
    polarisation: "**Vertical, omnidirectional in azimuth.** Half a vertical dipole standing on a \
                   metal plane, with the plane's mirror image supplying the other half. The pattern \
                   is the top half of the dipole's doughnut, which is where the extra 3 dB comes \
                   from: the same power squeezed into half the space.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let len = c.P("radiator", 0.2385 * lam);
    let plate = 0.25 * lam;
    Output {
        spec: "~5 dBi over a large plate · vertical · 36 Ω".into(),
        rows: vec![
            row("**radiator**, from the plate to the tip", c.fmt(len)),
            row("ground plane, minimum radius", c.fmt(plate)),
            total("total wire needed", c.fmt(len)),
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
        solve: Geometry::Wire(
            WireGeometry::new(vec![vec![[0.0; 3], [0.0, 0.0, len]]], [0.0; 3]).ground(0.0),
        ),
        diagram: Sketch::new()
            .ground(0.0)
            .wire(&[(0.0, 0.0), (0.0, len)], GOLD, 3.5)
            .dim((-0.08 * len, 0.0), (-0.08 * len, len), "radiator", (-8.0, 4.0), Anchor::End)
            .caption("equal in all horizontal directions, nothing along the plane", GREEN)
            .feed((0.0, 0.0), false)
            .render(),
        feed: vertical(
            "radiator, on the centre pin",
            "plate or roof, on the shield",
            Some("A bulkhead or NMO mount through the plate does both at once."),
        ),
        notes: "**The textbook vertical.** A quarter-wave wire on the centre pin, with the \
                connector body bonded to a sheet of metal: a car roof, a biscuit tin lid, a \
                square of PCB. The plate is the other half of the antenna.\n\n**The 5 dBi is \
                the ideal.** It assumes a plate that goes on forever. A plate a quarter \
                wavelength in radius gets you most of the way; a car roof at VHF is close to \
                ideal, at HF nothing is. Over real ground instead of metal, the losses in the \
                earth eat several dB, which is why ground-mounted HF verticals lay down dozens \
                of radials.\n\n**36 Ω is fine.** That is 1.4:1 on 50 Ω coax, less than 0.1 dB \
                of loss. Trim for resonance and leave it. If you want 50 Ω, bend radials down \
                instead of using a plate: that is the ground plane antenna."
            .into(),
        cut: None,
    }
}
