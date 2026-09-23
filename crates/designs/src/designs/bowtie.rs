use crate::draw::{Anchor, DIM, Drawing, GOLD, GREY, Stroke, hexa};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, CutFile, Design, Group, Output, Scene, row, total};
use antenna_solver::geometry::{Geometry, SurfaceGeometry};
use antenna_solver::surface::mesh::mesh_profile;

pub static BOWTIE: Design = Design {
    id: "bowtie",
    name: "Bowtie",
    group: Group::TwoSide,
    build: Build::Sheet,
    gain: "2 dBi",
    controls: &[ControlId::Flare],
    polarisation: "**Linear, along the axis of the bow.** Same doughnut as a dipole, and for the \
                   same reason: it is a dipole that got fat. Fatness buys bandwidth, not gain, so \
                   expect dipole-like numbers over a range where a dipole would have given up. Lay \
                   the axis horizontal for horizontal polarisation, which is what most TV \
                   transmitters use.",
    compute,
};

fn profile(x: f64, gap: f64, neck: f64, flare_tan: f64) -> f64 {
    (neck / 2.0).max((x.abs() - gap / 2.0) * flare_tan)
}

fn cut_of(arm: f64, gap: f64, neck: f64, flare_tan: f64, fmt: &dyn Fn(f64) -> String) -> CutFile {
    let tip = gap / 2.0 + arm;
    let half_width = arm * flare_tan;
    let knee = tip.min(gap / 2.0 + neck / 2.0 / flare_tan);
    let wing = |s: f64| {
        vec![
            [0.0, -neck / 2.0, 0.0],
            [s * knee, -neck / 2.0, 0.0],
            [s * tip, -half_width, 0.0],
            [s * tip, half_width, 0.0],
            [s * knee, neck / 2.0, 0.0],
            [0.0, neck / 2.0, 0.0],
        ]
    };
    CutFile {
        loops: vec![wing(-1.0), wing(1.0)],
        circles: vec![],
        note: format!(
            "Two wings, {} from the seam to the tip and {} across the open end, each blunted to a \
             {} tab that runs to the centre line. They meet there and the connector bridges the \
             join: pin to one tab, shield to the other. Do not leave metal across the seam.",
            fmt(tip),
            fmt(2.0 * half_width),
            fmt(neck)
        ),
    }
}

fn diagram(arm: f64, gap: f64, flare: f64, half: f64) -> Drawing {
    let w = 640.0;
    let pad = 60.0;
    let tip = gap / 2.0 + arm;
    let sc = (w - 2.0 * pad) / (2.0 * tip);
    let hh = half * sc;
    let h = 2.0 * hh + 2.0 * pad;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let xg = gap / 2.0 * sc;
    let xt = tip * sc;
    let mut g = Drawing::new(w, h);
    for s in [-1.0, 1.0] {
        g.polygon(
            &[
                (cx + s * xg, cy - 2.0),
                (cx + s * xt, cy - hh),
                (cx + s * xt, cy + hh),
                (cx + s * xg, cy + 2.0),
            ],
            Some(hexa(0xe8b23a, 0.18)),
            Some(Stroke::new(GOLD, 2.5)),
        );
    }
    g.dot(cx - xg, cy, 3.0, GOLD);
    g.dot(cx + xg, cy, 3.0, GREY);
    g.feed_flag(cx, cy, false);
    g.dim(cx + xg, cy + hh + 26.0, cx + xt, cy + hh + 26.0, "arm", 0.0, -6.0, Anchor::Middle);
    g.dim(cx + xt + 22.0, cy - hh, cx + xt + 22.0, cy + hh, "width", 26.0, 5.0, Anchor::Middle);
    g.label(cx, cy - hh - 18.0, format!("flare {flare:.0}°"), DIM, 12.0, Anchor::Middle);
    g.beam_label(cx, h - 12.0, Some("beam is broadside, out of the page both ways"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let arm = c.P("arm", 0.25 * lam);
    let gap = c.P("gap", lam / 120.0);
    let neck = c.P("neck width", lam / 90.0);
    let flare = c.ctl(ControlId::Flare);
    let flare_tan = (flare / 2.0).to_radians().tan();
    let tip = gap / 2.0 + arm;
    let half_width = profile(tip, gap, neck, flare_tan);
    let stations = (((2.0 * tip / (lam / 40.0)).round() as usize) * 2).max(8);
    let mesh = mesh_profile(-tip, tip, stations, |x| profile(x, gap, neck, flare_tan), 0.0, 4);
    Output {
        spec: "wideband · sheet metal · solved with RWG".into(),
        rows: vec![
            row("**arm** length, apex to tip", c.fmt(arm)),
            row("tip to tip overall", c.fmt(2.0 * tip)),
            row("**flare** angle", format!("{flare:.0}°")),
            row("width at the open end", c.fmt(2.0 * half_width)),
            row("**gap** at the feed", c.fmt(gap)),
            row("neck width holding it together", c.fmt(neck)),
            total("triangles in the model", mesh.triangles.len().to_string()),
        ],
        scene: Scene {
            mesh: Some(mesh.clone()),
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pol: "polarisation: linear, along the bow axis".into(),
            ..Default::default()
        },
        solve: Geometry::Surface(SurfaceGeometry {
            mesh,
            feed: [0.0; 3],
            feed_dir: [1.0, 0.0, 0.0],
            feed_tol: 2.0 * tip / stations as f64 * 0.3,
        }),
        diagram: diagram(arm, gap, flare, half_width),
        feed: inline(
            "left wing, on the centre pin",
            "right wing, on the shield",
            Some(
                "Cut the wings as two pieces and let the connector bridge them at the seam. The \
                 neck in the model is a tab on each wing, not metal across the feed.",
            ),
        ),
        notes: "**A dipole that got fat, and the easiest sheet antenna to cut.** Two triangles, \
                apexes facing each other across the feed gap, snipped from copper clad, flashing or \
                a biscuit tin. There is nothing to tune and nothing to match: the flare angle sets \
                the impedance and the arm length sets where the useful range starts.\n\n**Wide, not \
                high gain.** A bowtie is about 2 dBi, the same as the dipole it replaces, but it \
                holds that over an octave or more where a dipole covers a few percent. That is the \
                whole trade, and it is why the design survives on rooftops for UHF television.\n\n\
                **Flare angle is the one real choice.** Narrow flare behaves more like a plain \
                dipole, narrow band and nearer 70 Ω. Wide flare broadens the match and raises \
                impedance. 90° is the usual compromise; change it above and watch the solved SWR \
                curve stretch or collapse. A bowtie in free space sits well above 50 Ω, so this is \
                a design that genuinely wants a balun or a 2:1 transformer if you care about the \
                last decibel."
            .into(),
        cut: Some(cut_of(arm, gap, neck, flare_tan, c.fmt)),
    }
}
