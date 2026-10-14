use antenna_designs::draw::GOLD;
use antenna_designs::{Scene, wire};
use antenna_solver::fdtd::{Model, Part, Port, Stuff, Triangle};
use antenna_solver::geometry::Mesh;
use antenna_solver::vec::{Vec3, length, lerp, sub};
use egui::Ui;
use egui_bench::prelude::*;
use std::collections::HashMap;

pub const UNITS: [(&str, f64); 4] = [("mm", 1.0), ("cm", 10.0), ("m", 1000.0), ("inch", 25.4)];

#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub raw: Vec<Triangle>,
    pub unit: f64,
    pub feed: [Vec3; 2],
}

fn weld(triangles: &[Triangle]) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let mut index: HashMap<[u64; 3], usize> = HashMap::new();
    let mut points = Vec::new();
    let tris = triangles
        .iter()
        .map(|t| {
            t.map(|p| {
                *index.entry(p.map(f64::to_bits)).or_insert_with(|| {
                    points.push(p);
                    points.len() - 1
                })
            })
        })
        .collect();
    (points, tris)
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn parts(points: &[Vec3], tris: &[[usize; 3]]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..points.len()).collect();
    for t in tris {
        for k in 1..3 {
            let (a, b) = (root(&mut parent, t[0]), root(&mut parent, t[k]));
            parent[a] = b;
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..points.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by_key(|g| std::cmp::Reverse(g.len()));
    out
}

pub fn gap_feed(triangles: &[Triangle]) -> Option<[Vec3; 2]> {
    let (points, tris) = weld(triangles);
    let groups = parts(&points, &tris);
    if groups.len() < 2 {
        return None;
    }
    let thin = |g: &Vec<usize>| -> Vec<usize> {
        let step = g.len().div_ceil(3000).max(1);
        g.iter().step_by(step).copied().collect()
    };
    let (a, b) = (thin(&groups[0]), thin(&groups[1]));
    let nearest = |i: usize, other: &[usize]| {
        other.iter().map(|&j| length(sub(points[i], points[j]))).fold(f64::INFINITY, f64::min)
    };
    let gap = a.iter().map(|&i| nearest(i, &b)).fold(f64::INFINITY, f64::min);
    let face = |mine: &[usize], other: &[usize]| {
        let near: Vec<Vec3> = mine
            .iter()
            .filter(|&&i| nearest(i, other) <= gap * (1.0 + 1e-6) + 1e-12)
            .map(|&i| points[i])
            .collect();
        let n = near.len().max(1) as f64;
        near.iter().fold([0.0; 3], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n])
    };
    Some([face(&a, &b), face(&b, &a)])
}

impl Solid {
    pub fn new(raw: Vec<Triangle>) -> (Solid, bool) {
        let auto = gap_feed(&raw);
        let feed = auto.unwrap_or_else(|| {
            let low = raw
                .iter()
                .flatten()
                .copied()
                .min_by(|p, q| p[2].total_cmp(&q[2]))
                .unwrap_or([0.0; 3]);
            [[low[0], low[1], low[2] - 1.0], low]
        });
        (Solid { raw, unit: 1.0, feed }, auto.is_some())
    }

    pub fn triangles(&self) -> Vec<Triangle> {
        self.raw.iter().map(|t| t.map(|p| p.map(|v| v * self.unit))).collect()
    }

    pub fn feed_point(&self) -> Vec3 {
        lerp(self.feed[0], self.feed[1], 0.5)
    }

    pub fn key(&self) -> String {
        let sum: f64 = self.raw.iter().flatten().flatten().sum();
        format!("{}|{sum:.9e}|{}|{:?}", self.raw.len(), self.unit, self.feed)
    }

    pub fn model(&self, f0_hz: f64, span: f64) -> Model {
        let m = |p: Vec3| p.map(|v| v * 1e-3);
        Model {
            parts: vec![Part {
                triangles: self.triangles().into_iter().map(|t| t.map(m)).collect(),
                stuff: Stuff::Metal,
            }],
            metal: Vec::new(),
            wires: Vec::new(),
            dielectrics: Vec::new(),
            port: Some(Port { a: m(self.feed[0]), b: m(self.feed[1]), r: 50.0 }),
            f0: f0_hz,
            span,
            fine: 0.0,
            ground_plane_z: None,
        }
    }

    pub fn scene(&self, name: &str, beam: Option<Vec3>) -> Scene {
        let (vertices, triangles) = weld(&self.triangles());
        let feed = self.feed_point();
        Scene {
            mesh: Some(Mesh { vertices, triangles }),
            wires: vec![wire(self.feed.to_vec(), GOLD, 3.0)],
            feed: Some(feed),
            pattern_origin: Some(feed),
            beam_vec: beam,
            pol: format!("{name} · {} triangles · FDTD", self.raw.len()),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        }
    }

    pub fn editor(&mut self, ui: &mut Ui) -> bool {
        let mut back = false;
        ui.horizontal(|ui| {
            ui.label(legend("file units"));
            let name = UNITS.iter().find(|u| u.1 == self.unit).map_or("mm", |u| u.0);
            egui::ComboBox::from_id_salt("solid-units").selected_text(name).show_ui(ui, |ui| {
                for (label, scale) in UNITS {
                    if ui.selectable_label(self.unit == scale, label).clicked()
                        && self.unit != scale
                    {
                        let r = scale / self.unit;
                        self.feed = self.feed.map(|p| p.map(|v| v * r));
                        self.unit = scale;
                    }
                }
            });
            if ui.button(action("back to wires")).clicked() {
                back = true;
            }
        });
        egui::Grid::new("solid-feed").num_columns(4).spacing([6.0, 3.0]).show(ui, |ui| {
            for (label, p) in [("feed from", 0), ("feed to", 1)] {
                ui.label(legend(label));
                for v in &mut self.feed[p] {
                    ui.add(egui::DragValue::new(v).speed(0.1).max_decimals(3));
                }
                ui.end_row();
            }
        });
        hint(
            ui,
            "The mesh is metal. A 50 Ω port joins the two feed points, in mm; an import that holds two separate pieces puts it across the narrowest gap between them.",
        );
        back
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(x0: f64, x1: f64) -> Vec<Triangle> {
        let (y, z) = (0.5, 0.5);
        let p = |x: f64, a: f64, b: f64| [x, a, b];
        let quad = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| [[a, b, c], [a, c, d]];
        let mut out = Vec::new();
        out.extend(quad(p(x0, -y, -z), p(x1, -y, -z), p(x1, y, -z), p(x0, y, -z)));
        out.extend(quad(p(x0, -y, z), p(x0, y, z), p(x1, y, z), p(x1, -y, z)));
        out.extend(quad(p(x0, -y, -z), p(x0, -y, z), p(x1, -y, z), p(x1, -y, -z)));
        out.extend(quad(p(x0, y, -z), p(x1, y, -z), p(x1, y, z), p(x0, y, z)));
        out.extend(quad(p(x0, -y, -z), p(x0, y, -z), p(x0, y, z), p(x0, -y, z)));
        out.extend(quad(p(x1, -y, -z), p(x1, -y, z), p(x1, y, z), p(x1, y, -z)));
        out
    }

    #[test]
    fn two_bars_get_a_feed_across_their_gap() {
        let mut mesh = bar(-50.0, -1.5);
        mesh.extend(bar(1.5, 50.0));
        let (solid, auto) = Solid::new(mesh);
        assert!(auto);
        let gap = length(sub(solid.feed[0], solid.feed[1]));
        assert!((gap - 3.0).abs() < 1e-9, "{gap}");
        assert!(solid.feed.iter().all(|p| p[0].abs() == 1.5 && p[1] == 0.0 && p[2] == 0.0));
    }

    #[test]
    fn one_piece_has_no_gap_to_feed() {
        let (solid, auto) = Solid::new(bar(0.0, 10.0));
        assert!(!auto);
        assert_eq!(solid.feed[1][2], -0.5);
    }

    #[test]
    fn units_scale_the_mesh_and_the_model_is_in_metres() {
        let (mut solid, _) = Solid::new(bar(0.0, 10.0));
        solid.unit = 25.4;
        let model = solid.model(1e9, 0.1);
        let b = model.parts[0].bounds().unwrap();
        assert!((b.hi[0] - 0.254).abs() < 1e-12, "{:?}", b);
    }

    #[test]
    fn a_dipole_drawn_as_two_bars_resonates_near_half_a_wavelength() {
        use antenna_solver::fdtd::{Options, gpu, run, run_gpu};
        let mut mesh = bar(-71.0, -1.5);
        mesh.extend(bar(1.5, 71.0));
        let (solid, _) = Solid::new(mesh);
        let model = solid.model(1e9, 0.2);
        let opts = Options { far_field: false, ..Options::default() };
        let out = match gpu::block_on(gpu::adapter()) {
            Some(_) => gpu::block_on(run_gpu(&model, &opts, |_, _| true)).unwrap(),
            None => run(&model, &opts, |_, _| true).unwrap(),
        };
        let zs = &out.sweep.z;
        let k =
            (1..zs.len()).find(|k| zs[k - 1].im < 0.0 && zs[*k].im >= 0.0).expect("a resonance");
        let f = out.sweep.freqs[k];
        eprintln!("resonance {:.1} MHz, R {:.1} ohm", f / 1e6, zs[k].re);
        assert!((f / 0.97e9 - 1.0).abs() < 0.05, "{f}");
        assert!((40.0..110.0).contains(&zs[k].re), "{}", zs[k].re);
    }
}
