use crate::draw::{Anchor, DIM, Drawing, GOLD, GREEN};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;

pub static COLLINEAR: Design = Design {
    id: "collinear",
    name: "Collinear",
    group: Group::Omni,
    build: Build::Wire,
    gain: "6 dBi",
    controls: &[ControlId::Sections, ControlId::Phasing],
    polarisation: "**Vertical, omnidirectional, and flatter the taller you build it.** This is the \
                   stick on the roof: half-wave sections stacked end to end, all radiating in step, \
                   so the doughnut gets squashed toward the horizon. It has no more power than a \
                   dipole, it just stops wasting it on the sky and the ground.",
    compute,
};

fn diagram(
    n: usize,
    rad: f64,
    arm: f64,
    d: f64,
    phased: bool,
    fmt: &dyn Fn(f64) -> String,
) -> Drawing {
    let (w, h) = (640.0, 560.0);
    let total = n as f64 * rad + (n - 1) as f64 * d;
    let sc = 430.0 / total;
    let bot = h - 74.0;
    let cx = 250.0;
    let y_of = |v: f64| bot - v * sc;
    let arm_px = (arm * sc).max(40.0);
    let mut g = Drawing::new(w, h);
    let mut y = 0.0;
    for i in 0..n {
        g.line(cx, y_of(y), cx, y_of(y + rad), GOLD, 3.5);
        y += rad;
        if i < n - 1 {
            if phased {
                g.polyline(
                    &[
                        (cx, y_of(y)),
                        (cx + arm_px, y_of(y)),
                        (cx + arm_px, y_of(y + d)),
                        (cx, y_of(y + d)),
                    ],
                    GOLD,
                    2.4,
                );
            }
            y += d;
        }
    }
    g.dot(cx, y_of(rad / 2.0), 4.0, GREEN);
    g.label(
        cx + 16.0,
        y_of(rad / 2.0) + 4.0,
        "SMA, centre of the bottom section",
        GREEN,
        12.0,
        Anchor::Start,
    );
    g.dim(cx - 90.0, y_of(0.0), cx - 90.0, y_of(rad), "L", -8.0, 4.0, Anchor::End);
    g.dim(cx - 150.0, y_of(0.0), cx - 150.0, y_of(total), "H", -8.0, 4.0, Anchor::End);
    let joint = y_of(rad + d / 2.0);
    if phased {
        g.line(cx + arm_px + 8.0, joint, cx + arm_px + 64.0, joint - 20.0, DIM, 1.0);
        g.mono(
            cx + arm_px + 68.0,
            joint - 22.0,
            format!("P arm, {}", fmt(arm)),
            DIM,
            13.0,
            Anchor::Start,
        );
        g.mono(
            cx + arm_px + 68.0,
            joint - 6.0,
            format!("S gap, {}", fmt(d)),
            DIM,
            13.0,
            Anchor::Start,
        );
    } else {
        g.label(
            cx + 40.0,
            joint,
            "no phasing: sections fight each other",
            GOLD,
            12.0,
            Anchor::Start,
        );
    }
    g.label(
        cx,
        h - 14.0,
        if phased {
            "equal all round, squashed toward the horizon"
        } else {
            "equal all round, but the beam points at the sky"
        },
        GREEN,
        13.0,
        Anchor::Middle,
    );
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let n = c.ctl(ControlId::Sections) as usize;
    let phased = c.ctl(ControlId::Phasing) == 0.0;
    let rad = c.P("L section", 0.5 * lam);
    let arm = c.P("P phasing arm", 0.25 * lam);
    let d = c.P("S hairpin gap", lam / 60.0);
    let total_h = n as f64 * rad + (n - 1) as f64 * d;
    let mut pts: Vec<Vec3> = vec![[0.0; 3]];
    let mut y = 0.0;
    for i in 0..n {
        y += rad;
        pts.push([0.0, y, 0.0]);
        if i < n - 1 {
            if phased {
                pts.extend([[arm, y, 0.0], [arm, y + d, 0.0], [0.0, y + d, 0.0]]);
            } else {
                pts.push([0.0, y + d, 0.0]);
            }
            y += d;
        }
    }
    let ceiling = 10.0 * (2.0 * total_h / lam).log10();
    let mut rows = vec![
        row("**L** each radiating section", c.fmt(rad)),
        row("sections stacked", n.to_string()),
    ];
    if phased {
        rows.push(row("**P** phasing arm, each of two", c.fmt(arm)));
        rows.push(row("**S** gap between hairpin arms", c.fmt(d)));
    }
    rows.extend([
        row("**H** total height", c.fmt(total_h)),
        row("height in wavelengths", format!("{:.2} λ", total_h / lam)),
        row("line-source estimate, 2H/λ", format!("{ceiling:.1} dBi")),
        total("total wire", c.fmt(total_h + if phased { (n - 1) as f64 * 2.0 * arm } else { 0.0 })),
    ]);
    Output {
        spec: if phased {
            format!("{n} sections · vertical · high Z, needs matching")
        } else {
            format!("{n} sections · phasing removed · beam goes up")
        },
        rows,
        scene: Scene {
            wires: vec![wire(pts.clone(), GOLD, 3.0)],
            feed: Some([0.0, rad / 2.0, 0.0]),
            omni: true,
            omni_y: Some(total_h / 2.0),
            pol: "polarisation: vertical, omnidirectional in azimuth".into(),
            pattern_origin: Some([0.0, total_h / 2.0, 0.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(vec![pts], [0.0, rad / 2.0, 0.0])),
        diagram: diagram(n, rad, arm, d, phased, c.fmt),
        feed: vertical(
            "upper half of the bottom section, on the centre pin",
            "lower half, on the shield",
            Some(
                "The feed sits at a high impedance point, so it wants a matching section. See the \
                 feed and matching panel.",
            ),
        ),
        notes: "**Gain here is redistribution, not amplification.** A stack this tall cannot \
                radiate more power than you put in, so every decibel it claims on the horizon comes \
                off the sky and the ground. Height is the only thing that buys it: a vertical line \
                of wire tends toward 2H/λ once it is a few wavelengths tall, which is the estimate \
                in the table. Treat that as a guide rather than a limit, since it is an \
                approximation for long arrays and reads low for short ones: it gives 0 dBi for a \
                plain dipole, which really manages 2.15.\n\n**The hairpins are the whole trick.** A \
                half-wave section reverses the direction of its current halfway along, so if you \
                simply joined sections end to end, neighbouring sections would radiate against each \
                other. Each hairpin is a quarter wave out and a quarter wave back, which adds half \
                a wavelength of path and flips the phase, and because its two arms sit close \
                together carrying opposite currents, the hairpin itself radiates almost nothing. \
                Switch the phasing control off and watch the main lobe climb toward the sky: same \
                wire, same length, pointing at nothing useful.\n\n**Expect a high, awkward \
                impedance.** Feeding the bottom section puts you a few hundred ohms from 50, and the \
                current in the upper sections is weaker than in the lower ones, which is why a real \
                stack falls a little short of the theoretical ceiling. Make the hairpin arms \
                accurately: they are the one dimension that must be a true quarter wave, and \
                getting them wrong tilts the beam rather than just detuning it.\n\nOne warning \
                about the gain figure: it is the peak of the pattern, wherever that peak happens to \
                point. Switch the phasing off and the number barely moves, or even rises, while the \
                antenna becomes useless to talk to anyone on the ground. Always read it beside the \
                beam angle, not on its own."
            .into(),
        cut: None,
    }
}
