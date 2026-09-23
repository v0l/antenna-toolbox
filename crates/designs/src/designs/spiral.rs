use crate::draw::{Anchor, BOOM, DIM, Drawing, GOLD, Stroke};
use crate::export::{P2, flat, sample_curve};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, CutFile, Design, Group, Output, Scene, row, total};
use antenna_solver::geometry::{Geometry, Mesh, SurfaceGeometry};
use antenna_solver::surface::mesh::{feed_edges, topology};
use antenna_solver::units::C;
use antenna_solver::vec::Vec3;
use std::f64::consts::PI;

pub static SPIRAL: Design = Design {
    id: "spiral",
    name: "Archimedean spiral",
    group: Group::TwoSide,
    build: Build::Sheet,
    gain: "4 dBi",
    controls: &[ControlId::SpiralTurns],
    polarisation: "**Circular, and it stays circular right across the band.** Current chases its \
                   way around the arms rather than sloshing back and forth along them, so the \
                   field comes out rotating. Wind the arms the other way and you swap the hand. \
                   The catch is that a bare spiral fires equally from both faces, right hand \
                   circular out of one and left hand out of the other, so until you put a cavity \
                   behind it you are throwing half the power away. Against a linear antenna you \
                   give up the usual 3 dB, and against the wrong hand you give up everything.",
    compute,
};

const ACROSS: usize = 2;
const FEED_TOL: f64 = 1e-6;

fn sweep_strip(centres: &[P2], width: f64, across: usize, z: f64) -> Mesh {
    let n = centres.len();
    let ny = across.max(1);
    let mut vertices = Vec::with_capacity(n * (ny + 1));
    for i in 0..n {
        let c = centres[i];
        let (mut tx, mut ty) = (0.0, 0.0);
        for (from, to) in [(i as isize - 1, i as isize), (i as isize, i as isize + 1)] {
            if from < 0 || to as usize >= n {
                continue;
            }
            let (f, t) = (centres[from as usize], centres[to as usize]);
            let (dx, dy) = (t[0] - f[0], t[1] - f[1]);
            let dl = dx.hypot(dy).max(f64::MIN_POSITIVE);
            tx += dx / dl;
            ty += dy / dl;
        }
        let tl = if tx.hypot(ty) > 0.0 { tx.hypot(ty) } else { 1.0 };
        let hx = -ty / tl * width / 2.0;
        let hy = tx / tl * width / 2.0;
        for j in 0..=ny {
            let s = 2.0 * j as f64 / ny as f64 - 1.0;
            vertices.push([c[0] + hx * s, c[1] + hy * s, z]);
        }
    }
    let at = |i: usize, j: usize| i * (ny + 1) + j;
    let mut triangles = Vec::new();
    for i in 0..n - 1 {
        for j in 0..ny {
            if (i + j) % 2 == 0 {
                triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            } else {
                triangles.push([at(i, j), at(i + 1, j), at(i, j + 1)]);
                triangles.push([at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
    }
    Mesh { vertices, triangles }
}

#[derive(Clone, Copy)]
struct Spec {
    growth: f64,
    rho0: f64,
    theta_max: f64,
    step: f64,
    stand_off: f64,
    width: f64,
}

fn walk(s: &Spec, phase: f64) -> Vec<P2> {
    let arm = |t: f64| -> P2 {
        let rho = s.rho0 + s.growth * t;
        [rho * t.cos(), rho * t.sin()]
    };
    let min_gap = 1e-3;
    let mut thetas = vec![0.0];
    let mut t = phase * (s.step / s.rho0);
    while t < s.theta_max - min_gap {
        if t > thetas[thetas.len() - 1] + min_gap {
            thetas.push(t);
        }
        t += (s.step / (s.rho0 + s.growth * t)).min(0.35);
    }
    thetas.push(s.theta_max);

    let inbound: Vec<P2> = thetas
        .iter()
        .rev()
        .map(|&th| {
            let p = arm(th);
            [-p[0], -p[1]]
        })
        .collect();
    let diagonal = (2.0 * s.rho0).hypot(2.0 * s.stand_off);
    let segments = (2 * (diagonal / (2.0 * s.step)).round() as usize).max(4);
    let mut bridge = vec![[-s.rho0, s.stand_off]];
    for m in 1..segments {
        let u = m as f64 / segments as f64;
        bridge.push([-s.rho0 + 2.0 * s.rho0 * u, s.stand_off - 2.0 * s.stand_off * u]);
    }
    bridge.push([s.rho0, -s.stand_off]);
    inbound.into_iter().chain(bridge).chain(thetas.iter().map(|&t| arm(t))).collect()
}

struct Built {
    centres: Vec<P2>,
    mesh: Mesh,
    feed_dir: Vec3,
    arc_length: f64,
}

fn build(s: &Spec, lam: f64) -> Built {
    let diagonal = (2.0 * s.rho0).hypot(2.0 * s.stand_off);
    let feed_dir = [2.0 * s.rho0 / diagonal, -2.0 * s.stand_off / diagonal, 0.0];
    let mut best: Option<Built> = None;
    for attempt in 0..6 {
        let centres = walk(s, attempt as f64 * 0.17);
        let mesh = sweep_strip(&centres, s.width, ACROSS, 0.0);
        let arc_length =
            centres.windows(2).map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1])).sum();
        let driven = feed_edges(&topology(&mesh), [0.0; 3], feed_dir, FEED_TOL * lam);
        let candidate = Built { centres, mesh, feed_dir, arc_length };
        if driven.len() == ACROSS {
            return candidate;
        }
        best.get_or_insert(candidate);
    }
    best.expect("at least one attempt")
}

fn cut_of(s: &Spec, fmt: &dyn Fn(f64) -> String) -> CutFile {
    let half = s.width / 2.0;
    let edge = |offset: f64| {
        move |t: f64| -> P2 {
            let rho = s.rho0 + s.growth * t + offset;
            [rho * t.cos(), rho * t.sin()]
        }
    };
    let inner = edge(-half);
    let (mut lo, mut hi) = (0.0, PI / 2.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if inner(mid)[1] < half {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let arm = |sign: f64| {
        let mut pts: Vec<P2> = vec![[0.0, -half], [s.rho0 + half, -half]];
        pts.extend(sample_curve(edge(half), 0.0, s.theta_max, 0.5));
        pts.extend(sample_curve(inner, s.theta_max, hi, 0.5));
        pts.push([0.0, half]);
        flat(&pts.iter().map(|p| [sign * p[0], sign * p[1]]).collect::<Vec<_>>())
    };
    CutFile {
        loops: vec![arm(1.0), arm(-1.0)],
        circles: vec![],
        note: format!(
            "Two arms, {} wide with a {} gap between turns, drawn in place. Each root runs \
             straight in to the centre as a {} strip and the two meet there: that seam is the \
             feed, pin to one, shield to the other, and nothing else may cross it. Cut it from \
             copper clad or vinyl; the gap matters as much as the metal.",
            fmt(s.width),
            fmt(PI * s.growth - s.width),
            fmt(s.width)
        ),
    }
}

fn diagram(centres: &[P2], r_outer: f64, width: f64) -> Drawing {
    let size = 600.0;
    let pad = 56.0;
    let sc = (size / 2.0 - pad) / r_outer;
    let (cx, cy) = (size / 2.0, size / 2.0);
    let mut g = Drawing::new(size, size + 26.0);
    g.circle(cx, cy, r_outer * sc, None, Some(Stroke::dashed(BOOM, 1.0, 4.0, 5.0)));
    g.stroke_path(
        centres.iter().map(|p| [(cx + p[0] * sc) as f32, (cy - p[1] * sc) as f32]).collect(),
        false,
        Stroke::new(GOLD, (width * sc).max(1.5) as f32),
    );
    g.feed_flag(cx, cy, false);
    g.dim(cx, cy, cx + r_outer * sc, cy, "outer radius", 0.0, -9.0, Anchor::Middle);
    g.label(
        cx,
        cy - r_outer * sc - 16.0,
        "metal width equals the gap, which is what holds the impedance still",
        DIM,
        12.0,
        Anchor::Middle,
    );
    g.beam_label(cx, size + 18.0, Some("beam is broadside, out of the page both ways"));
    g
}

fn as_freq(mhz: f64) -> String {
    if mhz >= 1000.0 { format!("{:.2} GHz", mhz / 1000.0) } else { format!("{mhz:.0} MHz") }
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let turns = c.ctl(ControlId::SpiralTurns);
    let theta_max = 2.0 * PI * turns;
    let freq = C / lam;
    let r_outer = c.P("outer radius", 0.25 * lam);
    let self_comp = PI * r_outer / (2.0 * theta_max + 2.5 * PI);
    let width = c.P("arm width", self_comp);
    let rho0 = 2.0 * width;
    let spec = Spec {
        width,
        rho0,
        theta_max,
        stand_off: width,
        growth: ((r_outer - 2.5 * width) / theta_max).max(width / 8.0),
        step: lam / 18.0,
    };
    let built = build(&spec, lam);
    let r_inner = rho0 - width / 2.0;
    let gap = PI * spec.growth - width;
    Output {
        spec: "frequency independent · circular · solved with RWG".into(),
        rows: vec![
            row("**outer radius**", c.fmt(r_outer)),
            row("**turns** per arm", format!("{turns:.1}")),
            row("**arm width**", c.fmt(width)),
            row("gap between neighbouring arms", c.fmt(gap)),
            row("radius gained per turn", c.fmt(2.0 * PI * spec.growth)),
            row("inner radius at the feed", c.fmt(r_inner)),
            row("low corner, outer turn is one λ around", as_freq(freq * (2.0 * PI * r_outer / lam))),
            row("high corner, inner turn is one λ around", as_freq(freq * (2.0 * PI * r_inner / lam))),
            row("metal length, tip to tip", c.fmt(built.arc_length)),
            total("triangles in the model", built.mesh.triangles.len().to_string()),
        ],
        scene: Scene {
            mesh: Some(built.mesh.clone()),
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pol: "polarisation: circular, broadside, both faces".into(),
            ..Default::default()
        },
        solve: Geometry::Surface(SurfaceGeometry {
            mesh: built.mesh.clone(),
            feed: [0.0; 3],
            feed_dir: built.feed_dir,
            feed_tol: FEED_TOL * lam,
        }),
        diagram: diagram(&built.centres, r_outer, width),
        feed: inline(
            "one arm root, on the centre pin",
            "other arm root, on the shield",
            Some(
                "The two roots face each other across the middle, so the feed is a millimetre or \
                 two of wire at the exact centre. Bring the coax away perpendicular to the sheet, \
                 never across the face of it.",
            ),
        ),
        notes: "**The one antenna here that barely cares what frequency you give it.** A spiral \
                radiates from whichever ring of its own surface happens to be one wavelength \
                around, so when the frequency climbs, the active ring simply moves inward and \
                everything else carries on as before. Nothing resonates and nothing is cut to \
                length. The outer turn sets the low corner and the gap at the centre sets the high \
                one, and between those two the pattern, the polarisation and the impedance hardly \
                move.\n\n**Equal metal and equal gap is the whole trick.** A shape that looks the \
                same as its own negative is self complementary, and Babinet says such a thing has \
                to sit near 188 Ω in free space whatever else you do to it. That is the flat \
                impedance people mean when they call a spiral frequency independent. Change the \
                arm width above and you break the condition on purpose: the solved impedance starts \
                wandering with frequency, which is worth seeing once.\n\n**Cutting it is the hard \
                part.** This is a job for a PCB, a vinyl cutter, or a lot of patience with a \
                scalpel and copper tape. The gap matters as much as the metal, so a ragged cut \
                turns into a lumpy impedance curve, and the two arms have to match each other or \
                the polarisation goes elliptical. Keep the arm roots close together at the centre, \
                because that spacing is what limits the top of the band.\n\n**Two things the model \
                does not show you.** A bare spiral radiates from both faces, so half the power goes \
                out of the back until you add a cavity about a quarter wavelength deep at midband, \
                and a plain metal cavity hands back some of the bandwidth the spiral just bought \
                you. And 188 Ω into 50 Ω coax is nearly 4:1, so a spiral genuinely needs a balun, \
                usually a tapered one, which is often more work than the antenna itself."
            .into(),
        cut: Some(cut_of(&spec, c.fmt)),
    }
}
