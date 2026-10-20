use crate::drawing::colour;
use antenna_designs::Scene;
use antenna_solver::mom::Pattern;
use antenna_solver::vec::{Vec3, ring};
use egui::{Align2, Color32, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use egui_bench::prelude::*;
use egui_bench::viewer3d::{self, Camera, Lines, Look, Orbit, Style, Surface, V3, Viewer};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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

const MODEL: usize = 0;
const PATTERN: usize = 1;
const TINTED: usize = 2;
const SHADES: usize = 20;

pub struct View {
    pub camera: Camera,
    pub spin: bool,
    pub show: Show,
    orbit: Orbit,
    built: Option<(u64, Arc<viewer3d::Scene>)>,
}

fn start() -> Camera {
    Camera { yaw: 0.7, pitch: 0.3, ..Camera::default() }
}

impl Default for View {
    fn default() -> Self {
        Self { camera: start(), spin: true, show: Show::Both, orbit: Orbit::default(), built: None }
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

fn hsl(h: f32, s: f32, l: f32) -> V3 {
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
    [r + m, g + m, b + m]
}

struct Frame {
    z_up: bool,
}

impl Frame {
    fn of(scene: &Scene) -> Frame {
        Frame { z_up: scene.up()[2] > 0.5 }
    }

    fn place(&self, p: Vec3) -> V3 {
        let p = p.map(|v| v as f32);
        if self.z_up { p } else { [p[0], -p[2], p[1]] }
    }
}

fn rgb(c: Color32) -> V3 {
    viewer3d::rgb(c)
}

fn shade(alpha: u8) -> usize {
    TINTED + ((alpha as usize * SHADES + 127) / 255).min(SHADES - 1)
}

fn normal(p: &[V3]) -> V3 {
    let mut n = [0.0f32; 3];
    for k in 0..p.len() {
        let (a, b) = (p[k], p[(k + 1) % p.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    viewer3d::norm(n)
}

fn fan(group: usize, colour: V3, p: &[V3]) -> Surface {
    let n = normal(p);
    let mut s = Surface { colour, group, ..Default::default() };
    for k in 1..p.len().saturating_sub(1) {
        s.positions.extend([p[0], p[k], p[k + 1]]);
        s.normals.extend([n; 3]);
    }
    s
}

fn outline(group: usize, colour: V3, width: f32, p: &[V3], closed: bool) -> Lines {
    let mut segments: Vec<[V3; 2]> = p.windows(2).map(|w| [w[0], w[1]]).collect();
    if closed && p.len() > 2 {
        segments.push([p[p.len() - 1], p[0]]);
    }
    Lines { segments, colour, group, width, ..Default::default() }
}

fn signature(scene: &Scene, mesh: Option<&[Quad]>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut put = |v: f64| v.to_bits().hash(&mut h);
    for w in &scene.wires {
        w.p.iter().flatten().for_each(|v| put(*v));
        put(w.w as f64 + if w.thin { 0.5 } else { 0.0 });
        put(u32::from_be_bytes(w.c) as f64);
    }
    for p in &scene.polys {
        p.p.iter().flatten().for_each(|v| put(*v));
        put(p.fill.map(u32::from_be_bytes).unwrap_or(1) as f64);
        put(p.stroke.map(u32::from_be_bytes).unwrap_or(1) as f64);
    }
    if let Some(m) = &scene.mesh {
        m.vertices.iter().flatten().for_each(|v| put(*v));
        put(m.triangles.len() as f64);
    }
    scene.pattern_origin.iter().flatten().for_each(|v| put(*v));
    put(scene.up()[2]);
    for q in mesh.unwrap_or_default() {
        q.p.iter().flatten().for_each(|v| put(*v));
    }
    put(mesh.map(|m| m.len() as f64).unwrap_or(-1.0));
    h.finish()
}

fn build(scene: &Scene, mesh: Option<&[Quad]>, bounds: &Bounds) -> viewer3d::Scene {
    let frame = Frame::of(scene);
    let mut out = viewer3d::Scene {
        centre: frame.place(bounds.centre),
        radius: (bounds.radius * 1.4) as f32,
        ..Default::default()
    };
    if let Some(mesh) = mesh {
        let o = scene.pattern_origin.unwrap_or([0.0; 3]);
        for q in mesh {
            let p = q.p.map(|v| frame.place([v[0] + o[0], v[1] + o[1], v[2] + o[2]]));
            let hue = 214.0 - 180.0 * q.v;
            let light = 0.22 + 0.30 * q.v;
            out.surfaces.push(fan(PATTERN, hsl(hue, 0.68, light), &p));
            out.lines.push(outline(PATTERN, hsl(hue, 0.60, light + 0.12), 0.6, &p, true));
        }
    }
    if let Some(m) = &scene.mesh {
        let fill = Color32::from_rgba_unmultiplied(232, 178, 58, 51);
        for t in &m.triangles {
            let p = t.map(|i| frame.place(m.vertices[i]));
            out.surfaces.push(fan(shade(fill.a()), rgb(fill), &p));
            out.lines.push(outline(MODEL, rgb(fill), 0.6, &p, true));
        }
    }
    for poly in &scene.polys {
        let p: Vec<V3> = poly.p.iter().map(|q| frame.place(*q)).collect();
        let fill =
            poly.fill.map(colour).unwrap_or(Color32::from_rgba_unmultiplied(89, 101, 109, 77));
        let stroke = poly.stroke.map(colour).unwrap_or(Color32::from_rgb(0x59, 0x65, 0x6d));
        let group = if fill.a() == 255 { MODEL } else { shade(fill.a()) };
        out.surfaces.push(fan(group, rgb(fill), &p));
        out.lines.push(outline(MODEL, rgb(stroke), 1.0, &p, true));
    }
    for wire in &scene.wires {
        let p: Vec<V3> = wire.p.iter().map(|q| frame.place(*q)).collect();
        let width = wire.w * if wire.thin { 0.6 } else { 1.0 };
        out.lines.push(outline(MODEL, rgb(colour(wire.c)), width, &p, false));
    }
    out
}

fn looks(show: Show) -> Vec<Look> {
    let model = show != Show::Pattern;
    let pattern = match show {
        Show::Wire => 0.0,
        Show::Pattern => 1.0,
        Show::Both => 0.5,
    };
    let mut looks = vec![Look::HIDDEN; TINTED + SHADES];
    looks[MODEL] = if model { Look::default() } else { Look::HIDDEN };
    looks[PATTERN] = Look { opacity: pattern, ..Look::default() };
    for k in 0..SHADES {
        let opacity = if model { (k as f32 / SHADES as f32).max(0.05) } else { 0.0 };
        looks[TINTED + k] = Look { opacity, ..Look::default() };
    }
    looks
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
    let key = signature(scene, mesh);
    let built = match &view.built {
        Some((k, s)) if *k == key => s.clone(),
        _ => {
            let s = Arc::new(build(scene, mesh, &bounds));
            view.built = Some((key, s.clone()));
            s
        }
    };
    let shown = if mesh.is_some() { view.show } else { Show::Wire };
    if resp.double_clicked() {
        view.camera = start();
    }
    let style = Style { background: WELL, ..Style::default() };
    let mut viewer = Viewer::new(built, view.camera).looks(looks(shown)).style(style);
    if viewer.navigate(ui, &resp, &mut view.orbit) && resp.dragged() {
        view.spin = false;
    }
    view.camera = viewer.camera();
    if view.spin {
        view.camera.yaw += ui.input(|i| i.stable_dt).min(0.1) * 0.25;
        ui.ctx().request_repaint();
    }
    viewer.paint(ui, rect);
    let frame = Frame::of(scene);
    let projector = viewer.projector(rect);
    let at = |q: Vec3| projector.project(frame.place(q));
    overlays(ui, rect, scene, &bounds, &frame, &at, status);
}

fn overlays(
    ui: &Ui,
    rect: egui::Rect,
    scene: &Scene,
    bounds: &Bounds,
    frame: &Frame,
    at: &dyn Fn(Vec3) -> Option<Pos2>,
    status: &str,
) {
    let p = ui.painter_at(rect);
    let green = OK;
    let label_font = theme::legend_font(12.0);
    if let Some(q) = scene.feed.and_then(at) {
        p.circle_filled(q, 5.0, green);
        p.text(q + Vec2::new(9.0, 0.0), Align2::LEFT_CENTER, "SMA", label_font.clone(), green);
    }
    if scene.omni {
        let rr = bounds.radius * 1.15;
        let c = bounds.centre;
        let pts: Vec<Pos2> = ring(64, |a| {
            if frame.z_up {
                [c[0] + rr * a.cos(), c[1] + rr * a.sin(), scene.omni_y.unwrap_or(c[2])]
            } else {
                [c[0] + rr * a.cos(), scene.omni_y.unwrap_or(c[1]), c[2] + rr * a.sin()]
            }
        })
        .into_iter()
        .filter_map(at)
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
        let tip = [base[0] + bv[0] * r, base[1] + bv[1] * r, base[2] + bv[2] * r];
        if let (Some(o), Some(t)) = (at(base), at(tip)) {
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
        if ui.add(Key::new("reset").outline(VALUE)).clicked() {
            view.camera = start();
            view.spin = true;
        }
        Line::new()
            .note("drag to orbit · shift-drag to pan · scroll to zoom · double-click to reset")
            .size(10.5)
            .show(ui);
    });
}
