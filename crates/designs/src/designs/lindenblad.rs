use crate::draw::{Anchor, DIM, Drawing, GOLD, GREEN, SILVER, Stroke, hex};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, Wire, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;
use std::f64::consts::PI;

pub static LINDENBLAD: Design = Design {
    id: "lindenblad",
    name: "Parasitic Lindenblad",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2 dBi",
    controls: &[ControlId::Cant, ControlId::Hand],
    polarisation: "**Circular, and omnidirectional with it.** That combination is the point: a \
                   satellite tumbles, so its polarisation wanders, and a circular antenna never \
                   drops into the deep null a linear one gives you. It costs a nominal 3 dB against \
                   a perfectly aligned linear antenna and saves you far more than that on a bad \
                   pass. The axial ratio beside the gain figure is how circular the model thinks it \
                   actually is, where 0 dB is perfect.",
    compute,
};

fn diagram(driven: f64, para: f64, radius: f64, cant: f64, fmt: &dyn Fn(f64) -> String) -> Drawing {
    let (w, h) = (640.0, 420.0);
    let (cx, cy) = (300.0, 210.0);
    let sc = 150.0 / (radius + para / 2.0);
    let rr = radius * sc;
    let dv = driven / 2.0 * sc;
    let t = cant.to_radians();
    let mut g = Drawing::new(w, h);
    g.arc(cx, cy, rr, rr * 0.34, 0.0, 2.0 * PI, Stroke::dashed(hex(0x233139), 1.0, 4.0, 4.0));
    for i in 0..4 {
        let ph = i as f64 * PI / 2.0;
        let x = cx + rr * ph.cos();
        let yc = cy + rr * ph.sin() * 0.34;
        let half = para / 2.0 * sc;
        let dx = -ph.sin() * t.cos() * half;
        let dy = -t.sin() * half;
        g.line(x - dx, yc - dy, x + dx, yc + dy, SILVER, 2.6);
    }
    g.line(cx, cy - dv, cx, cy - 5.0, GOLD, 4.0);
    g.line(cx, cy + 5.0, cx, cy + dv, GOLD, 4.0);
    g.dot(cx, cy, 3.5, GREEN);
    g.dim(cx - 44.0, cy - dv, cx - 44.0, cy + dv, "A", -8.0, 4.0, Anchor::End);
    g.dim(
        cx,
        cy + rr * 0.34 + 26.0,
        cx + rr,
        cy + rr * 0.34 + 26.0,
        "R",
        0.0,
        16.0,
        Anchor::Middle,
    );
    g.label(
        cx + rr + 30.0,
        cy - 46.0,
        format!("4 parasitic wires, {} each", fmt(para)),
        SILVER,
        12.0,
        Anchor::Start,
    );
    g.mono(cx + rr + 30.0, cy - 28.0, format!("canted {cant:.0}°"), DIM, 13.0, Anchor::Start);
    g.label(cx - rr - 30.0, cy + dv + 26.0, "driven dipole, vertical", GOLD, 12.0, Anchor::End);
    g.label(
        cx,
        h - 12.0,
        "circular polarisation, equal all round, strongest near the horizon",
        GREEN,
        13.0,
        Anchor::Middle,
    );
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let cant = c.ctl(ControlId::Cant);
    let right = c.ctl(ControlId::Hand) == 0.0;
    let driven = c.P("A driven dipole", 0.475 * lam);
    let para = c.P("P parasitic wire", 0.455 * lam);
    let radius = c.P("R ring radius", 0.15 * lam);
    let t = cant.to_radians();
    let sense = if right { 1.0 } else { -1.0 };
    let mut lines: Vec<Vec<Vec3>> = vec![vec![[0.0, -driven / 2.0, 0.0], [0.0, driven / 2.0, 0.0]]];
    let mut wires: Vec<Wire> = vec![wire(lines[0].clone(), GOLD, 3.5)];
    for i in 0..4 {
        let ph = i as f64 * PI / 2.0;
        let c0 = [radius * ph.cos(), 0.0, radius * ph.sin()];
        let d = [-ph.sin() * t.cos(), sense * t.sin(), ph.cos() * t.cos()];
        let seg = vec![
            [c0[0] - d[0] * para / 2.0, c0[1] - d[1] * para / 2.0, c0[2] - d[2] * para / 2.0],
            [c0[0] + d[0] * para / 2.0, c0[1] + d[1] * para / 2.0, c0[2] + d[2] * para / 2.0],
        ];
        wires.push(wire(seg.clone(), SILVER, 2.6));
        lines.push(seg);
    }
    Output {
        spec: "circular · omni · AA2TX parasitic Lindenblad".into(),
        rows: vec![
            row("**A** driven dipole, tip to tip", c.fmt(driven)),
            row("**P** parasitic wire, each of four", c.fmt(para)),
            row("**R** ring radius from the centre", c.fmt(radius)),
            row("cant from horizontal", format!("{cant:.0}°")),
            row("sense", if right { "right hand (RHCP)" } else { "left hand (LHCP)" }),
            total("total wire", c.fmt(driven + 4.0 * para)),
        ],
        scene: Scene {
            wires,
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(0.0),
            pol: format!("polarisation: circular, {}", if right { "right hand" } else { "left hand" }),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(lines, [0.0; 3])),
        diagram: diagram(driven, para, radius, cant, c.fmt),
        feed: vertical(
            "upper half of the driven dipole, on the centre pin",
            "lower half, on the shield",
            Some(
                "The four canted wires connect to nothing at all. They are parasitic: the driven \
                 dipole lights them up.",
            ),
        ),
        notes: "**One driven element, four that are not connected to anything.** A proper \
                Lindenblad drives four canted dipoles in phase, which means four feedlines, four \
                baluns and a matching harness. Tony Monteiro AA2TX worked out in 2006 that you can \
                get the same pattern from a single vertical dipole surrounded by four parasitic \
                wires. They pick up energy from the driven element, and because each one lies at \
                an angle, the current forced along it radiates both a vertical and a horizontal \
                component, a quarter cycle apart. Add that to the driven dipole's vertical field \
                and you have circular polarisation from one coax.\n\n**The parasitic length is the \
                tuning, and it is sharp.** Those wires have to be the length that makes their \
                induced current land 180 degrees behind the driven element, and the best length \
                moves with wire thickness, because a fatter wire behaves electrically longer. Watch \
                the axial ratio figure and trim the parasitics, not the driven element.\n\n**The \
                sense comes out backwards, which trips people.** To radiate right hand circular, the \
                wires must be canted the way a left hand Lindenblad would be, because the trick \
                works by cancelling half the driven element's vertical field and that inverts the \
                sense. Set the hand control and read the label beside the gain rather than working \
                it out from the drawing.\n\nGain is modest and that is the design working as \
                intended: it spreads power evenly around the horizon instead of concentrating it. \
                AA2TX measured 7.47 dBic at 3 degrees elevation with the antenna ten feet over real \
                ground, where the ground reflection adds several dB. This model is in free space \
                with no ground, so expect a lower number here."
            .into(),
        cut: None,
    }
}
