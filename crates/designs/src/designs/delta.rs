use crate::draw::{Anchor, GOLD, GREEN};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::{Vec3, lerp};

pub static DELTA: Design = Design {
    id: "delta",
    name: "Delta loop",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "3.0 dBi",
    controls: &[ControlId::LoopFeed],
    polarisation: "**Set by where you feed it.** Fed in the middle of the bottom wire it is \
                   horizontally polarised; fed a quarter wave down from the apex, on a sloping \
                   side, it is vertical. Either way it beams broadside to the loop, front and back.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let side_feed = c.ctl(ControlId::LoopFeed) == 1.0;
    let per = c.P("perimeter", 1.096 * lam);
    let s = per / 3.0;
    let hgt = s * 3f64.sqrt() / 2.0;
    let (l, r, apex): (Vec3, Vec3, Vec3) =
        ([-s / 2.0, 0.0, 0.0], [s / 2.0, 0.0, 0.0], [0.0, hgt, 0.0]);
    let feed = if side_feed { lerp(apex, r, 0.75) } else { [0.0; 3] };
    let pts = vec![l, [0.0; 3], r, apex, l];
    Output {
        spec: format!(
            "~3 dBi · {} · about 120 Ω, 75 Ω quarter-wave line to 50 Ω",
            if side_feed { "vertical" } else { "horizontal" }
        ),
        rows: vec![
            row("**each side**", c.fmt(s)),
            row("height, base to apex", c.fmt(hgt)),
            row(
                "feed point",
                if side_feed {
                    format!("on a side, {} down from the apex", c.fmt(0.75 * s))
                } else {
                    "middle of the bottom wire".to_string()
                },
            ),
            total("total wire", c.fmt(per)),
        ],
        scene: Scene {
            wires: vec![wire(pts.clone(), GOLD, 3.5)],
            feed: Some(feed),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([0.0, hgt / 3.0, 0.0]),
            pol: if side_feed { "polarisation: vertical" } else { "polarisation: horizontal" }
                .into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(vec![pts], feed)),
        diagram: Sketch::new()
            .wire(&[(-s / 2.0, 0.0), (s / 2.0, 0.0), (0.0, hgt), (-s / 2.0, 0.0)], GOLD, 3.0)
            .dim((-s / 2.0, -0.06 * s), (s / 2.0, -0.06 * s), "side", (0.0, 14.0), Anchor::Middle)
            .caption("broadside to the loop, front and back", GREEN)
            .feed((feed[0], feed[1]), !side_feed)
            .render(),
        feed: inline("one side of the gap", "other side of the gap", None),
        notes: "**A full-wave loop bent into a triangle.** About 1 dB better than a dipole, \
                quieter on receive because it is a closed loop, and it hangs from a single \
                high point with the base stretched out below. The favourite HF single-element \
                antenna for people with one tall tree.\n\n**Impedance about 120 Ω.** A quarter \
                wave of 75 Ω coax transforms that to near 50, which the matching section works \
                out. Apex up or apex down makes little difference to the pattern; which side \
                you feed it on decides the polarisation, and vertical polarisation is the one \
                for low-angle DX over good ground."
            .into(),
        cut: None,
    }
}
