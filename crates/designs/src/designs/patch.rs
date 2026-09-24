use crate::draw::{Anchor, Drawing, GOLD, GREEN, GREY, SILVER, Stroke, hexa};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Poly, SUBSTRATES, Scene, row, total};
use antenna_solver::fdtd::{Aabb, Dielectric, Model, Port};
use antenna_solver::geometry::{Geometry, Mesh};

pub static PATCH: Design = Design {
    id: "patch",
    name: "Microstrip patch",
    group: Group::Beam,
    build: Build::Pcb,
    gain: "6 dBi",
    controls: &[ControlId::Substrate, ControlId::Thickness],
    polarisation: "**Linear, along the patch length, with the beam straight off the board.** One \
                   broad lobe on the copper side, about 70° wide, and nothing much behind the \
                   ground plane. Turn the board a quarter turn and the polarisation turns with it.",
    compute,
};

pub struct Board {
    pub eps: f64,
    pub tan: f64,
    pub h: f64,
    pub name: &'static str,
}

pub fn board(c: &Ctx) -> Board {
    let s = SUBSTRATES[(c.ctl(ControlId::Substrate) as usize).min(SUBSTRATES.len() - 1)];
    Board { name: s.0, eps: s.1, tan: s.2, h: c.ctl(ControlId::Thickness) }
}

pub fn rect(lo: [f64; 3], hi: [f64; 3]) -> Mesh {
    Mesh {
        vertices: vec![
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [hi[0], hi[1], hi[2]],
            [lo[0], hi[1], hi[2]],
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

pub fn merge(parts: &[Mesh]) -> Mesh {
    let mut out = Mesh { vertices: Vec::new(), triangles: Vec::new() };
    for m in parts {
        let base = out.vertices.len();
        out.vertices.extend_from_slice(&m.vertices);
        out.triangles.extend(m.triangles.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }
    out
}

pub fn outline(lo: [f64; 2], hi: [f64; 2], z: f64) -> Poly {
    Poly {
        p: vec![[lo[0], lo[1], z], [hi[0], lo[1], z], [hi[0], hi[1], z], [lo[0], hi[1], z]],
        fill: Some(hexa(0x2f6b3a, 0.35)),
        stroke: Some(hexa(0x6fbf73, 0.6)),
    }
}

pub fn m(v: f64) -> f64 {
    v / 1000.0
}

fn diagram(l: f64, w: f64, feed: f64, gl: f64, gw: f64) -> Drawing {
    let (dw, dh) = (640.0, 420.0);
    let sc = 300.0 / gl.max(gw);
    let (cx, cy) = (300.0, 210.0);
    let mut g = Drawing::new(dw, dh);
    g.rect(
        cx - gl / 2.0 * sc,
        cy - gw / 2.0 * sc,
        gl * sc,
        gw * sc,
        Some(hexa(0x2f6b3a, 0.35)),
        Some(Stroke::new(SILVER, 1.5)),
    );
    g.rect(
        cx - l / 2.0 * sc,
        cy - w / 2.0 * sc,
        l * sc,
        w * sc,
        Some(hexa(0xe8b23a, 0.25)),
        Some(Stroke::new(GOLD, 2.5)),
    );
    g.circle(cx + feed * sc, cy, 4.0, Some(GOLD), None);
    g.label(cx + feed * sc + 8.0, cy - 8.0, "probe", GOLD, 11.0, Anchor::Start);
    g.dim(
        cx - l / 2.0 * sc,
        cy + w / 2.0 * sc + 20.0,
        cx + l / 2.0 * sc,
        cy + w / 2.0 * sc + 20.0,
        "length",
        0.0,
        -6.0,
        Anchor::Middle,
    );
    g.dim(
        cx - l / 2.0 * sc - 20.0,
        cy - w / 2.0 * sc,
        cx - l / 2.0 * sc - 20.0,
        cy + w / 2.0 * sc,
        "width",
        -8.0,
        4.0,
        Anchor::End,
    );
    g.dim(cx, cy - 20.0, cx + feed * sc, cy - 20.0, "feed", 0.0, -6.0, Anchor::Middle);
    g.label(
        cx + gl / 2.0 * sc + 10.0,
        cy + gw / 2.0 * sc - 6.0,
        "ground on the back",
        GREY,
        11.0,
        Anchor::Start,
    );
    g.label(cx, dh - 14.0, "beam straight out of the page", GREEN, 12.0, Anchor::Middle);
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let b = board(c);
    let (er, h) = (b.eps, b.h);
    let w0 = lam / 2.0 * (2.0 / (er + 1.0)).sqrt();
    let ee = (er + 1.0) / 2.0 + (er - 1.0) / 2.0 / (1.0 + 12.0 * h / w0).sqrt();
    let dl = 0.412 * h * (ee + 0.3) * (w0 / h + 0.264) / ((ee - 0.258) * (w0 / h + 0.8));
    let l0 = lam / (2.0 * ee.sqrt()) - 2.0 * dl;
    let k0h = 2.0 * std::f64::consts::PI * h / lam;
    let g1 = w0 / (120.0 * lam) * (1.0 - k0h * k0h / 24.0);
    let r_edge = 1.0 / (2.0 * g1);
    let inset = l0 / std::f64::consts::PI * (50.0 / r_edge).sqrt().min(1.0).acos();
    let l = c.P("patch length", l0);
    let w = c.P("patch width", w0);
    let feed = c.P("feed from centre", l0 / 2.0 - inset);
    let gl = c.P("board length", l0 + 12.0 * h + lam / 5.0);
    let gw = c.P("board width", w0 + 12.0 * h + lam / 5.0);
    let model = Model {
        metal: vec![
            Aabb::new([m(-gl / 2.0), m(-gw / 2.0), 0.0], [m(gl / 2.0), m(gw / 2.0), 0.0]),
            Aabb::new([m(-l / 2.0), m(-w / 2.0), m(h)], [m(l / 2.0), m(w / 2.0), m(h)]),
        ],
        dielectrics: vec![Dielectric {
            bounds: Aabb::new([m(-gl / 2.0), m(-gw / 2.0), 0.0], [m(gl / 2.0), m(gw / 2.0), m(h)]),
            eps_r: er,
            tan_d: b.tan,
        }],
        port: Some(Port { a: [m(feed), 0.0, 0.0], b: [m(feed), 0.0, m(h)], r: 50.0 }),
        f0: 299_792_458.0 / m(lam),
        span: 0.1,
        fine: 0.0,
        ground_plane_z: None,
    };
    let mesh = merge(&[rect([-l / 2.0, -w / 2.0, h], [l / 2.0, w / 2.0, h])]);
    Output {
        spec: format!("~6 dBi · printed on {} · solved with FDTD", b.name),
        rows: vec![
            row("**patch length**, along the feed", c.fmt(l)),
            row("**patch width**", c.fmt(w)),
            row("**probe**, from the patch centre toward an edge", c.fmt(feed)),
            row("probe from the nearer edge", c.fmt(l / 2.0 - feed)),
            row("board length", c.fmt(gl)),
            row("board width", c.fmt(gw)),
            row(format!("board, {} εr {:.2}", b.name, er), c.fmt(h)),
            total("copper to cut", format!("{} × {}", c.fmt(l), c.fmt(w))),
        ],
        scene: Scene {
            mesh: Some(mesh),
            polys: vec![outline([-gl / 2.0, -gw / 2.0], [gl / 2.0, gw / 2.0], 0.0)],
            feed: Some([feed, 0.0, h]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([0.0, 0.0, h]),
            up: Some([0.0, 0.0, 1.0]),
            pol: "polarisation: linear, along the patch length".into(),
            ..Default::default()
        },
        solve: Geometry::Volume(model),
        diagram: diagram(l, w, feed, gl, gw),
        feed: vertical(
            "patch, on the probe",
            "ground plane, on the shield",
            Some(
                "An SMA from the back: its flange soldered to the ground plane, its pin up \
                 through a hole in the board and soldered to the patch at the marked point.",
            ),
        ),
        notes: "**A resonant cavity that leaks from two edges.** The copper rectangle and the \
                ground plane under it form a half-wave cavity along the length; the fields \
                fringing out at the two ends are what radiate. That is why the width barely moves \
                the frequency but does set the impedance and the bandwidth, and why a thicker \
                board with a lower permittivity gives a wider, more efficient antenna.\n\n\
                **The probe position sets the match.** At the edge the patch looks like a few \
                hundred ohms; at the centre, zero. The probe sits where it crosses 50 Ω, found \
                from the edge conductance. Slide it toward the centre if the solved resistance \
                is high, outward if low.\n\n**The sizes come from the transmission-line model**, \
                which runs a couple of percent high on frequency for thin FR-4. The FDTD solve \
                shows where it really lands: read the dip in the SWR plot and scale the length by \
                that frequency over yours.\n\n**FR-4 costs about half the power** at 2.4 GHz \
                through its loss tangent. The efficiency figure is that loss, solved, not an \
                estimate."
            .into(),
        cut: None,
    }
}
