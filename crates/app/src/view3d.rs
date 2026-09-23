use crate::drawing::colour;
use antenna_designs::Scene;
use antenna_solver::mom::Pattern;
use antenna_solver::vec::{Vec3, ring};
use egui::{Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use egui_bench::prelude::*;

pub struct Quad {
    pub p: [Vec3; 4],
    pub v: f32,
}

pub fn pattern_mesh(f: &Pattern, r: f64) -> Vec<Quad> {
    let (nt, np) = (30usize, 40usize);
    let mut dirs = vec![vec![[0.0; 3]; np + 1]; nt + 1];
    let mut amp = vec![vec![0.0; np + 1]; nt + 1];
    let mut peak: f64 = 1e-12;
    for i in 0..=nt {
        let th = i as f64 / nt as f64 * std::f64::consts::PI;
        let (st, ct) = th.sin_cos();
        for j in 0..=np {
            let ph = j as f64 / np as f64 * std::f64::consts::TAU;
            let d = [st * ph.cos(), ct, st * ph.sin()];
            let a = f(d).abs();
            dirs[i][j] = d;
            amp[i][j] = a;
            peak = peak.max(a);
        }
    }
    let val = |a: f64| ((20.0 * (a.max(1e-9) / peak).log10() + 20.0) / 20.0).clamp(0.0, 1.0);
    let mut quads = Vec::new();
    for i in 0..nt {
        for j in 0..np {
            let idx = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let v = idx.iter().map(|&(a, b)| val(amp[a][b])).sum::<f64>() / 4.0;
            if v < 0.02 {
                continue;
            }
            let p = idx.map(|(a, b)| {
                let d = dirs[a][b];
                let rr = r * val(amp[a][b]);
                [d[0] * rr, d[1] * rr, d[2] * rr]
            });
            quads.push(Quad { p, v: v as f32 });
        }
    }
    quads
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Show {
    Wire,
    Pattern,
    Both,
}

pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: Vec2,
    pub spin: bool,
    pub show: Show,
}

impl Default for View {
    fn default() -> Self {
        Self { yaw: 0.7, pitch: 0.3, zoom: 1.0, pan: Vec2::ZERO, spin: true, show: Show::Both }
    }
}

pub struct Bounds {
    pub centre: Vec3,
    pub radius: f64,
}

pub fn bounds_of(scene: &Scene) -> Bounds {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let pts = scene
        .wires
        .iter()
        .flat_map(|w| w.p.iter())
        .chain(scene.polys.iter().flat_map(|p| p.p.iter()))
        .chain(scene.mesh.iter().flat_map(|m| m.vertices.iter()));
    for q in pts {
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    if !lo[0].is_finite() {
        return Bounds { centre: [0.0; 3], radius: 1.0 };
    }
    let centre = [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2.0);
    let radius = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max) / 2.0;
    Bounds { centre, radius: if radius > 0.0 { radius } else { 1.0 } }
}

struct Camera {
    cy: f64,
    sy: f64,
    cp: f64,
    sp: f64,
    dist: f64,
    focal: f64,
    origin: Pos2,
    centre: Vec3,
}

impl Camera {
    fn project(&self, p: Vec3) -> (Pos2, f64) {
        let (x, y, z) = (p[0] - self.centre[0], p[1] - self.centre[1], p[2] - self.centre[2]);
        let x1 = x * self.cy + z * self.sy;
        let z1 = -x * self.sy + z * self.cy;
        let y2 = y * self.cp - z1 * self.sp;
        let z2 = y * self.sp + z1 * self.cp;
        let f = self.focal / (self.dist + z2).max(self.focal * 0.05);
        (Pos2::new(self.origin.x + (x1 * f) as f32, self.origin.y - (y2 * f) as f32), z2)
    }
}

fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Color32::from_rgba_unmultiplied(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
        (a * 255.0) as u8,
    )
}

enum Item {
    Poly { z: f64, pts: Vec<Pos2>, fill: Color32, stroke: Color32 },
    Seg { z: f64, a: Pos2, b: Pos2, colour: Color32, width: f32 },
}

impl Item {
    fn z(&self) -> f64 {
        match self {
            Item::Poly { z, .. } | Item::Seg { z, .. } => *z,
        }
    }
}

pub fn show(
    ui: &mut Ui,
    scene: &Scene,
    mesh: Option<&[Quad]>,
    view: &mut View,
    height: f32,
    status: &str,
) {
    let bounds = bounds_of(scene);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::click_and_drag());
    if resp.dragged() {
        view.spin = false;
        let d = resp.drag_delta();
        if ui.input(|i| i.modifiers.shift) {
            view.pan += d;
        } else {
            view.yaw += d.x * 0.008;
            view.pitch = (view.pitch + d.y * 0.006).clamp(-1.45, 1.45);
        }
    }
    if resp.hovered() {
        let s = ui.input(|i| i.smooth_scroll_delta.y);
        if s != 0.0 {
            view.zoom = (view.zoom * (1.0 + s * 0.002)).clamp(0.3, 6.0);
        }
    }
    if view.spin {
        view.yaw += ui.input(|i| i.stable_dt).min(0.1) * 0.25;
        ui.ctx().request_repaint();
    }
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, WELL);
    let cam = Camera {
        cy: (view.yaw as f64).cos(),
        sy: (view.yaw as f64).sin(),
        cp: (view.pitch as f64).cos(),
        sp: (view.pitch as f64).sin(),
        dist: bounds.radius * 10.0,
        focal: f64::from(rect.width().min(rect.height())) * 4.0 * view.zoom as f64,
        origin: rect.center() + view.pan,
        centre: bounds.centre,
    };
    let show = if mesh.is_some() { view.show } else { Show::Wire };
    let mut items = Vec::new();
    if let (Some(mesh), true) = (mesh, show != Show::Wire) {
        let o = scene.pattern_origin.unwrap_or([0.0; 3]);
        let solo = show == Show::Pattern;
        for q in mesh {
            let pr: Vec<(Pos2, f64)> =
                q.p.iter().map(|v| cam.project([v[0] + o[0], v[1] + o[1], v[2] + o[2]])).collect();
            let hue = 214.0 - 180.0 * q.v;
            let light = 0.22 + 0.30 * q.v;
            items.push(Item::Poly {
                z: pr.iter().map(|x| x.1).sum::<f64>() / 4.0,
                pts: pr.iter().map(|x| x.0).collect(),
                fill: hsla(hue, 0.68, light, if solo { 0.92 } else { 0.5 }),
                stroke: hsla(hue, 0.60, light + 0.12, if solo { 0.5 } else { 0.22 }),
            });
        }
    }
    if show != Show::Pattern {
        if let Some(m) = &scene.mesh {
            for t in &m.triangles {
                let pr: Vec<(Pos2, f64)> = t.iter().map(|&i| cam.project(m.vertices[i])).collect();
                items.push(Item::Poly {
                    z: pr.iter().map(|x| x.1).sum::<f64>() / 3.0,
                    pts: pr.iter().map(|x| x.0).collect(),
                    fill: Color32::from_rgba_unmultiplied(232, 178, 58, 51),
                    stroke: Color32::from_rgba_unmultiplied(232, 178, 58, 140),
                });
            }
        }
        for poly in &scene.polys {
            let pr: Vec<(Pos2, f64)> = poly.p.iter().map(|&q| cam.project(q)).collect();
            items.push(Item::Poly {
                z: pr.iter().map(|x| x.1).sum::<f64>() / pr.len() as f64,
                pts: pr.iter().map(|x| x.0).collect(),
                fill: poly
                    .fill
                    .map(colour)
                    .unwrap_or(Color32::from_rgba_unmultiplied(89, 101, 109, 77)),
                stroke: poly.stroke.map(colour).unwrap_or(Color32::from_rgb(0x59, 0x65, 0x6d)),
            });
        }
        for wire in &scene.wires {
            let pr: Vec<(Pos2, f64)> = wire.p.iter().map(|&q| cam.project(q)).collect();
            for pair in pr.windows(2) {
                items.push(Item::Seg {
                    z: (pair[0].1 + pair[1].1) / 2.0,
                    a: pair[0].0,
                    b: pair[1].0,
                    colour: colour(wire.c),
                    width: wire.w * if wire.thin { 0.6 } else { 1.0 },
                });
            }
        }
    }
    items.sort_by(|a, b| b.z().total_cmp(&a.z()));
    let mut run = egui::Mesh::default();
    let mut edges: Vec<[Pos2; 2]> = Vec::new();
    let mut edge_colour = Vec::new();
    let flush =
        |run: &mut egui::Mesh, edges: &mut Vec<[Pos2; 2]>, edge_colour: &mut Vec<Color32>| {
            if !run.is_empty() {
                p.add(Shape::mesh(std::mem::take(run)));
            }
            for (e, c) in edges.drain(..).zip(edge_colour.drain(..)) {
                p.line_segment(e, Stroke::new(0.6, c));
            }
        };
    for it in items {
        match it {
            Item::Poly { pts, fill, stroke, .. } => {
                let base = run.vertices.len() as u32;
                for q in &pts {
                    run.colored_vertex(*q, fill);
                }
                for k in 1..pts.len() as u32 - 1 {
                    run.add_triangle(base, base + k, base + k + 1);
                }
                for k in 0..pts.len() {
                    edges.push([pts[k], pts[(k + 1) % pts.len()]]);
                    edge_colour.push(stroke);
                }
            }
            Item::Seg { z, a, b, colour, width } => {
                flush(&mut run, &mut edges, &mut edge_colour);
                let alpha = (1.0 - z / (bounds.radius * 5.0)).clamp(0.45, 1.0) as f32;
                p.line_segment([a, b], Stroke::new(width, colour.gamma_multiply(alpha)));
                p.circle_filled(a, width / 2.0, colour.gamma_multiply(alpha));
                p.circle_filled(b, width / 2.0, colour.gamma_multiply(alpha));
            }
        }
    }
    flush(&mut run, &mut edges, &mut edge_colour);
    let green = Color32::from_rgb(0x6f, 0xbf, 0x73);
    let label_font = FontId::proportional(12.0);
    if let Some(f) = scene.feed {
        let (q, _) = cam.project(f);
        p.circle_filled(q, 5.0, green);
        p.text(q + Vec2::new(9.0, 0.0), Align2::LEFT_CENTER, "SMA", label_font.clone(), green);
    }
    if scene.omni {
        let y = scene.omni_y.unwrap_or(bounds.centre[1]);
        let rr = bounds.radius * 1.15;
        let pts: Vec<Pos2> =
            ring(64, |a| [bounds.centre[0] + rr * a.cos(), y, bounds.centre[2] + rr * a.sin()])
                .into_iter()
                .map(|q| cam.project(q).0)
                .collect();
        p.extend(Shape::dashed_line(&pts, Stroke::new(1.0, green), 5.0, 5.0));
        if let Some(east) = pts.iter().max_by(|a, b| a.x.total_cmp(&b.x)) {
            p.text(*east + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, "omni", label_font, green);
        }
    } else {
        let bv = scene.beam_direction();
        let base = if bv[2] != 0.0 {
            [bounds.centre[0], bounds.centre[1], scene.beam_from.unwrap_or(bounds.centre[2])]
        } else {
            bounds.centre
        };
        let r = bounds.radius * 1.5;
        let (o, _) = cam.project(base);
        let (t, _) = cam.project([base[0] + bv[0] * r, base[1] + bv[1] * r, base[2] + bv[2] * r]);
        p.extend(Shape::dashed_line(&[o, t], Stroke::new(1.5, green), 5.0, 4.0));
        let dir = (t - o).normalized();
        let side = Vec2::new(-dir.y, dir.x);
        p.add(Shape::convex_polygon(
            vec![t, t - dir * 10.0 + side * 4.0, t - dir * 10.0 - side * 4.0],
            green,
            Stroke::NONE,
        ));
        p.text(t + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, "beam", label_font, green);
    }
    p.text(
        Pos2::new(rect.left() + 10.0, rect.bottom() - 10.0),
        Align2::LEFT_BOTTOM,
        &scene.pol,
        theme::figure(11.0),
        LEGEND,
    );
    p.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 8.0),
        Align2::LEFT_TOP,
        status,
        theme::figure(11.0),
        LEGEND,
    );
}

pub fn controls(ui: &mut Ui, view: &mut View) {
    ui.horizontal(|ui| {
        for (mode, name) in [(Show::Wire, "wire"), (Show::Pattern, "pattern"), (Show::Both, "both")]
        {
            if toggle(ui, name, view.show == mode).clicked() {
                view.show = mode;
            }
        }
        if toggle(ui, "spin", view.spin).clicked() {
            view.spin = !view.spin;
        }
        if ui.button(action("reset")).clicked() {
            let show = view.show;
            *view = View { show, ..View::default() };
        }
        Line::new().note("drag to orbit · scroll to zoom · shift-drag to pan").size(10.5).show(ui);
    });
}
