use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, RealGround, WireGeometry};

pub static INVV: Design = Design {
    id: "invv",
    name: "Inverted V",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "6.3 dBi",
    controls: &[ControlId::ApexAngle, ControlId::Height],
    polarisation: "**Mostly horizontal, broadside to the wire.** Solved over average ground, so \
                   the gain includes the ground reflection and its losses. The drooping arms fill \
                   in the dipole's end nulls a little and add some vertical polarisation off the \
                   ends.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let angle = c.ctl(ControlId::ApexAngle).to_radians();
    let apex = c.ctl(ControlId::Height) * lam;
    let arm = c.P("arm", 0.247 * lam);
    let (sx, sz) = ((angle / 2.0).sin() * arm, (angle / 2.0).cos() * arm);
    let tip = |s: f64| [s * sx, 0.0, apex - sz];
    let mut geo =
        WireGeometry::new(vec![vec![tip(-1.0), [0.0, 0.0, apex], tip(1.0)]], [0.0, 0.0, apex])
            .ground(0.0);
    geo.real_ground = Some(RealGround::AVERAGE);
    Output {
        spec: format!("~6 dBi over average ground · about 50 Ω · apex {:.2} λ up", apex / lam),
        rows: vec![
            row("**each arm**, apex to tip", c.fmt(arm)),
            row("apex height", c.fmt(apex)),
            row("tip height", c.fmt(apex - sz)),
            row("span between the tips", c.fmt(2.0 * sx)),
            total("total wire", c.fmt(2.0 * arm)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![tip(-1.0), [-0.01 * lam, 0.0, apex]], GOLD, 3.5),
                wire(vec![[0.01 * lam, 0.0, apex], tip(1.0)], GOLD, 3.5),
            ],
            feed: Some([0.0, 0.0, apex]),
            beam_vec: Some([0.0, 1.0, 0.0]),
            pattern_origin: Some([0.0, 0.0, apex]),
            pol: "polarisation: mostly horizontal, broadside".into(),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(geo),
        diagram: Sketch::new()
            .ground(0.0)
            .wire(&[(0.0, 0.0), (0.0, apex)], crate::draw::BOOM, 4.0)
            .wire(&[(-sx, apex - sz), (0.0, apex), (sx, apex - sz)], GOLD, 3.0)
            .dim((sx * 1.15, 0.0), (sx * 1.15, apex), "apex height", (8.0, 4.0), Anchor::Start)
            .note((-sx / 2.0, apex - sz / 2.0 + 0.02 * lam), "arm", GOLD, Anchor::End)
            .caption("broadside to the wire, strongest at a mid angle overhead", GREEN)
            .feed((0.0, apex), false)
            .render(),
        feed: inline("left arm", "right arm", Some("Hang the balun from the mast at the apex.")),
        notes: "**A dipole on one mast.** The centre goes up the pole, the ends come down to \
                short stakes or the fence. One support instead of two is the whole appeal, and \
                bending the arms down lowers the impedance from a flat dipole's 70 Ω or so to \
                about 50, so coax fits directly.\n\n**Height sets the pattern.** This one is \
                solved over average ground, with the angle and apex height as controls. Low \
                down it throws most of its power straight up, which suits short-range HF \
                work; above half a wave the main lobe comes down toward the horizon for \
                distance. Keep the tips out of reach: they carry high voltage when \
                transmitting.\n\nThe ground model is the reflection-coefficient approximation, \
                good for wires more than about a tenth of a wave up. Tips closer to the ground \
                than that are only roughly modelled."
            .into(),
        cut: None,
    }
}
