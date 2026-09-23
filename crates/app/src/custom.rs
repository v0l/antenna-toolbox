use antenna_designs::draw::{GOLD, SILVER};
use antenna_designs::{Poly, Scene, wire};
use antenna_solver::C64;
use antenna_solver::geometry::{Load, Network, RealGround, SolveLine, WireGeometry, WireProps};
use antenna_solver::nec;
use antenna_solver::vec::{Vec3, lerp};
use egui::Ui;
use egui_bench::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wire {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ground {
    Free,
    Perfect,
    Real(RealGround),
}

impl Ground {
    pub const PRESETS: [(&'static str, Ground); 7] = [
        ("free space", Ground::Free),
        ("perfect ground", Ground::Perfect),
        ("average ground", Ground::Real(RealGround::AVERAGE)),
        ("poor ground", Ground::Real(RealGround::POOR)),
        ("good ground", Ground::Real(RealGround::GOOD)),
        ("sea water", Ground::Real(RealGround::SEA)),
        ("fresh water", Ground::Real(RealGround::FRESH_WATER)),
    ];

    pub fn label(self) -> &'static str {
        Ground::PRESETS.iter().find(|(_, g)| *g == self).map(|(n, _)| *n).unwrap_or("real ground")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Custom {
    pub name: String,
    pub wires: Vec<Wire>,
    pub feed_wire: usize,
    pub feed_at: f64,
    pub ground: Ground,
    pub sources: Vec<(Vec3, C64)>,
    pub loads: Vec<(Vec3, Load)>,
    pub networks: Vec<(Vec3, Vec3, Network)>,
    pub path: String,
    pub message: Option<(bool, String)>,
}

impl Default for Custom {
    fn default() -> Self {
        Custom {
            name: "custom".into(),
            wires: vec![Wire { a: [0.0, -463.0, 3000.0], b: [0.0, 463.0, 3000.0], radius: 1.0 }],
            feed_wire: 0,
            feed_at: 0.5,
            ground: Ground::Free,
            sources: Vec::new(),
            loads: Vec::new(),
            networks: Vec::new(),
            path: String::new(),
            message: None,
        }
    }
}

impl Custom {
    pub fn key(&self) -> String {
        format!(
            "{:?}|{}|{}|{:?}|{:?}|{:?}|{:?}",
            self.wires,
            self.feed_wire,
            self.feed_at,
            self.ground,
            self.sources,
            self.loads,
            self.networks
        )
    }

    pub fn feed_point(&self) -> Vec3 {
        let w = self.wires.get(self.feed_wire).or(self.wires.first());
        w.map(|w| lerp(w.a, w.b, self.feed_at.clamp(0.0, 1.0))).unwrap_or([0.0; 3])
    }

    pub fn geometry(&self, props: WireProps) -> WireGeometry {
        let mut g = WireGeometry {
            lines: self
                .wires
                .iter()
                .map(|w| SolveLine {
                    pts: vec![w.a, w.b],
                    rad: Some(w.radius),
                    props,
                    segments: None,
                })
                .collect(),
            feed: self.feed_point(),
            sources: self.sources.clone(),
            loads: self.loads.clone(),
            networks: self.networks.clone(),
            ..Default::default()
        };
        match self.ground {
            Ground::Free => {}
            Ground::Perfect => g.ground_z = Some(0.0),
            Ground::Real(r) => {
                g.ground_z = Some(0.0);
                g.real_ground = Some(r);
                g.sommerfeld = true;
            }
        }
        g
    }

    pub fn scene(&self, beam: Option<Vec3>) -> Scene {
        let mut s = Scene {
            wires: self
                .wires
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    wire(vec![w.a, w.b], if i == self.feed_wire { GOLD } else { SILVER }, 3.0)
                })
                .collect(),
            feed: Some(self.feed_point()),
            pattern_origin: Some(self.feed_point()),
            beam_vec: beam,
            pol: format!("custom model · {} wires · {}", self.wires.len(), self.ground.label()),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        };
        if self.ground != Ground::Free {
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for w in &self.wires {
                for p in [w.a, w.b] {
                    lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                    hi = [hi[0].max(p[0]), hi[1].max(p[1])];
                }
            }
            let pad = ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.3).max(100.0);
            s.polys.push(Poly {
                p: vec![
                    [lo[0] - pad, lo[1] - pad, 0.0],
                    [hi[0] + pad, lo[1] - pad, 0.0],
                    [hi[0] + pad, hi[1] + pad, 0.0],
                    [lo[0] - pad, hi[1] + pad, 0.0],
                ],
                fill: Some([0x3a, 0x4a, 0x3c, 110]),
                stroke: Some([0x59, 0x65, 0x6d, 255]),
            });
        }
        s
    }

    pub fn from_geometry(
        name: &str,
        geo: &WireGeometry,
        default_radius: f64,
        base: f64,
    ) -> Result<Custom, String> {
        if !geo.mirrors.is_empty() || geo.po.is_some() {
            return Err(
                "this template uses reflector images or physical optics, which are not plain wires"
                    .into(),
            );
        }
        let turn = |p: Vec3| -> Vec3 {
            match geo.ground_z {
                Some(z) => [p[0], p[1], p[2] - z],
                None => [p[0], -p[2], p[1]],
            }
        };
        let lift = if geo.ground_z.is_none() {
            let lowest = geo
                .lines
                .iter()
                .flat_map(|l| &l.pts)
                .map(|&p| turn(p)[2])
                .fold(f64::INFINITY, f64::min);
            base - lowest
        } else {
            0.0
        };
        let at = |p: Vec3| {
            let q = turn(p);
            [q[0] + 0.0, q[1] + 0.0, q[2] + lift + 0.0]
        };
        let mut wires = Vec::new();
        let mut feed = (0, 0.5);
        let target = at(geo.feed);
        let mut best = f64::INFINITY;
        for l in &geo.lines {
            for pair in l.pts.windows(2) {
                let (a, b) = (at(pair[0]), at(pair[1]));
                let ab = antenna_solver::vec::sub(b, a);
                let len2 = antenna_solver::vec::dot(ab, ab);
                if len2 < 1e-12 {
                    continue;
                }
                let t = (antenna_solver::vec::dot(antenna_solver::vec::sub(target, a), ab) / len2)
                    .clamp(0.0, 1.0);
                let d =
                    antenna_solver::vec::length(antenna_solver::vec::sub(target, lerp(a, b, t)));
                if d < best {
                    best = d;
                    feed = (wires.len(), t);
                }
                wires.push(Wire { a, b, radius: l.rad.unwrap_or(default_radius) });
            }
        }
        Ok(Custom {
            name: name.to_string(),
            wires,
            feed_wire: feed.0,
            feed_at: feed.1,
            ground: match (geo.ground_z, geo.real_ground) {
                (None, _) => Ground::Free,
                (Some(_), None) => Ground::Perfect,
                (Some(_), Some(r)) => Ground::Real(r),
            },
            sources: geo.sources.iter().map(|&(p, v)| (at(p), v)).collect(),
            loads: geo.loads.iter().map(|&(p, l)| (at(p), l)).collect(),
            networks: geo.networks.iter().map(|&(a, b, n)| (at(a), at(b), n)).collect(),
            path: String::new(),
            message: None,
        })
    }

    pub fn import(&mut self, text: &str, default_radius: f64) -> Result<Option<f64>, String> {
        let imp = nec::import(text)?;
        let mut c = Custom::from_geometry(&self.name, &imp.geo, default_radius, 0.0)?;
        c.path = self.path.clone();
        let warn = imp.warnings.join("; ");
        c.message = Some((
            imp.warnings.is_empty(),
            format!(
                "imported {} wires{}",
                c.wires.len(),
                if warn.is_empty() { String::new() } else { format!(": {warn}") }
            ),
        ));
        *self = c;
        Ok(imp.freq_mhz)
    }

    pub fn editor(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(legend("ground"));
            let mut g = self.ground;
            egui::ComboBox::from_id_salt("custom-ground").selected_text(g.label()).show_ui(
                ui,
                |ui| {
                    for (name, preset) in Ground::PRESETS {
                        ui.selectable_value(&mut g, preset, name);
                    }
                },
            );
            self.ground = g;
            if let Ground::Real(r) = &mut self.ground {
                ui.label(legend("εr"));
                ui.add(egui::DragValue::new(&mut r.eps_r).range(1.0..=100.0).speed(0.1));
                ui.label(legend("σ S/m"));
                ui.add(
                    egui::DragValue::new(&mut r.sigma)
                        .range(0.0..=10.0)
                        .speed(0.0005)
                        .max_decimals(4),
                );
            }
        });
        if self.ground != Ground::Free {
            hint(
                ui,
                "The ground is the plane z = 0. Real ground uses Sommerfeld's exact solution, the same as NEC-2's GN 2, so wires close to the ground are handled properly.",
            );
        }
        ui.add_space(4.0);
        let mut remove = None;
        let mut duplicate = None;
        ui.horizontal(|ui| {
            let lowest =
                self.wires.iter().flat_map(|w| [w.a[2], w.b[2]]).fold(f64::INFINITY, f64::min);
            let mut height = lowest;
            ui.label(legend("lowest point z"));
            ui.add(egui::DragValue::new(&mut height).speed(5.0).max_decimals(1));
            if (height - lowest).abs() > 1e-9 {
                for w in &mut self.wires {
                    w.a[2] += height - lowest;
                    w.b[2] += height - lowest;
                }
            }
            Line::new()
                .note("mm above the ground plane; drag to raise or lower the whole antenna")
                .size(10.5)
                .show(ui);
        });
        egui::ScrollArea::both().id_salt("wires").max_height(320.0).show(ui, |ui| {
            egui::Grid::new("wire-table").num_columns(10).spacing([6.0, 3.0]).striped(true).show(
                ui,
                |ui| {
                    for h in ["#", "x1", "y1", "z1", "x2", "y2", "z2", "radius", "feed", ""] {
                        ui.label(legend(h));
                    }
                    ui.end_row();
                    for (i, w) in self.wires.iter_mut().enumerate() {
                        ui.label(value(format!("{}", i + 1)));
                        for v in w.a.iter_mut().chain(w.b.iter_mut()) {
                            ui.add(egui::DragValue::new(v).speed(1.0).max_decimals(2));
                        }
                        ui.add(
                            egui::DragValue::new(&mut w.radius)
                                .range(0.01..=500.0)
                                .speed(0.01)
                                .max_decimals(3),
                        );
                        if ui.radio(self.feed_wire == i, "").clicked() {
                            self.feed_wire = i;
                        }
                        ui.horizontal(|ui| {
                            if ui.small_button("copy").clicked() {
                                duplicate = Some(i);
                            }
                            if ui.small_button("✕").clicked() {
                                remove = Some(i);
                            }
                        });
                        ui.end_row();
                    }
                },
            );
        });
        if let Some(i) = duplicate {
            let w = self.wires[i];
            self.wires.insert(i + 1, w);
        }
        if let Some(i) = remove.filter(|_| self.wires.len() > 1) {
            self.wires.remove(i);
            if self.feed_wire >= self.wires.len() {
                self.feed_wire = self.wires.len() - 1;
            }
        }
        ui.horizontal(|ui| {
            if ui.button(action("add wire")).clicked() {
                let last = *self.wires.last().unwrap_or(&Wire {
                    a: [0.0; 3],
                    b: [0.0, 0.0, 100.0],
                    radius: 1.0,
                });
                self.wires.push(Wire {
                    a: last.b,
                    b: [last.b[0], last.b[1], last.b[2] + 100.0],
                    radius: last.radius,
                });
            }
            ui.label(legend("feed at"));
            ui.add(egui::Slider::new(&mut self.feed_at, 0.0..=1.0).fixed_decimals(3));
            Line::new()
                .note("of the way along the fed wire, from its first end")
                .size(10.5)
                .show(ui);
        });
        hint(
            ui,
            "Millimetres, z up. Wire ends that meet join electrically; a wire ending on the ground plane connects to it.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_turns_upright_and_keeps_its_feed() {
        let geo = WireGeometry::new(vec![vec![[0.0, -250.0, 0.0], [0.0, 250.0, 0.0]]], [0.0; 3]);
        let c = Custom::from_geometry("dipole", &geo, 1.0, 1000.0).unwrap();
        assert_eq!(c.wires.len(), 1);
        assert!((c.wires[0].a[2] - 1000.0).abs() < 1e-9 && (c.wires[0].b[2] - 1500.0).abs() < 1e-9);
        assert!((c.feed_point()[2] - 1250.0).abs() < 1e-9);
        assert_eq!(c.ground, Ground::Free);
    }
}
