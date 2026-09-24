use crate::worker::Job;
use antenna_rf::exposure::{Limits, Standard, limits};
use antenna_solver::near::NearSource;
use antenna_solver::vec::Vec3;
use egui::{Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2};
use egui_bench::prelude::*;
use rayon::prelude::*;
use std::sync::Arc;

const N: usize = 121;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plane {
    Across,
    AlongA,
    AlongB,
}

struct Grid {
    key: String,
    centre: Vec3,
    u: Vec3,
    v: Vec3,
    half: f64,
    e2: Vec<f64>,
    h2: Vec<f64>,
    dist: Vec<f64>,
    wires: Vec<(Vec3, Vec3)>,
}

pub struct NearPanel {
    pub watts: f64,
    pub duty: f64,
    pub standard: Standard,
    pub plane: Plane,
    pub half_m: f64,
    job: Option<Job<Grid>>,
    grid: Option<Grid>,
    tex: Option<(String, TextureHandle)>,
}

impl Default for NearPanel {
    fn default() -> Self {
        Self {
            watts: 25.0,
            duty: 50.0,
            standard: Standard::IcnirpPublic,
            plane: Plane::AlongA,
            half_m: 0.0,
            job: None,
            grid: None,
            tex: None,
        }
    }
}

fn axes(up: Vec3, plane: Plane) -> (Vec3, Vec3, &'static str) {
    let up_axis = (0..3).max_by(|a, b| up[*a].abs().total_cmp(&up[*b].abs())).unwrap_or(1);
    let others: Vec<usize> = (0..3).filter(|&i| i != up_axis).collect();
    let unit = |i: usize| {
        let mut v = [0.0; 3];
        v[i] = 1.0;
        v
    };
    let name = ["x", "y", "z"];
    match plane {
        Plane::Across => (unit(others[0]), unit(others[1]), "horizontal"),
        Plane::AlongA => (unit(others[0]), unit(up_axis), name[others[0]]),
        Plane::AlongB => (unit(others[1]), unit(up_axis), name[others[1]]),
    }
}

fn colour(db: f64) -> Color32 {
    let t = ((db + 30.0) / 50.0).clamp(0.0, 1.0) as f32;
    let stops: [(f32, [u8; 3]); 6] = [
        (0.0, [0x10, 0x14, 0x24]),
        (0.3, [0x1d, 0x4e, 0x89]),
        (0.5, [0x2a, 0x9d, 0x8f]),
        (0.6, [0xe9, 0xc4, 0x6a]),
        (0.8, [0xf4, 0xa2, 0x61]),
        (1.0, [0xe7, 0x4c, 0x3c]),
    ];
    let i = stops.iter().position(|s| s.0 >= t).unwrap_or(5).max(1);
    let (a, b) = (stops[i - 1], stops[i]);
    let f = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    let c = |k: usize| (a.1[k] as f32 + (b.1[k] as f32 - a.1[k] as f32) * f) as u8;
    Color32::from_rgb(c(0), c(1), c(2))
}

impl NearPanel {
    fn power(&self) -> f64 {
        self.watts * self.duty / 100.0
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        near: &Arc<NearSource>,
        up: Vec3,
        lam_mm: f64,
        freq: f64,
        dbi: f64,
    ) {
        let lim = limits(self.standard, freq * 1e6);
        section(
            ui,
            "near field and exposure",
            "time-averaged, against the reference levels",
            |ui| {
                self.controls(ui);
                let Some(lim) = lim else {
                    note(ui, "This standard has no reference level at this frequency.", LEGEND);
                    return;
                };
                let key = format!("{:p}|{:?}|{}", Arc::as_ptr(near), self.plane, self.half_m);
                if let Some(j) = &mut self.job
                    && let Some(g) = j.poll().pop()
                {
                    self.grid = Some(g);
                    self.job = None;
                }
                if self.grid.as_ref().is_none_or(|g| g.key != key) && self.job.is_none() {
                    self.start(ui.ctx(), near.clone(), up, lam_mm, key.clone());
                }
                let Some(g) = self.grid.take() else {
                    note(ui, "Computing the field…", LEGEND);
                    return;
                };
                self.plot(ui, &g, &lim);
                self.summary(ui, &g, &lim, dbi);
                self.grid = Some(g);
            },
        );
    }

    fn controls(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(legend("transmit W"));
            ui.add(egui::DragValue::new(&mut self.watts).range(0.001..=5000.0).speed(0.5));
            ui.label(legend("duty %"));
            ui.add(egui::DragValue::new(&mut self.duty).range(0.1..=100.0).speed(0.5));
            ui.label(legend("half width m"));
            ui.add(
                egui::DragValue::new(&mut self.half_m)
                    .range(0.0..=200.0)
                    .speed(0.05)
                    .custom_formatter(
                        |v, _| if v == 0.0 { "auto".into() } else { format!("{v:.2}") },
                    ),
            );
        });
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("exposure-standard")
                .selected_text(self.standard.label())
                .show_ui(ui, |ui| {
                    for s in Standard::ALL {
                        ui.selectable_value(&mut self.standard, s, s.label());
                    }
                });
            for (p, l) in [
                (Plane::Across, "horizontal"),
                (Plane::AlongA, "upright 1"),
                (Plane::AlongB, "upright 2"),
            ] {
                if toggle(ui, l, self.plane == p).clicked() {
                    self.plane = p;
                }
            }
        });
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        near: Arc<NearSource>,
        up: Vec3,
        lam_mm: f64,
        key: String,
    ) {
        let (u, v, _) = axes(up, self.plane);
        let wires: Vec<(Vec3, Vec3)> = near.wires().collect();
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (a, b) in &wires {
            for p in [a, b] {
                for t in 0..3 {
                    lo[t] = lo[t].min(p[t]);
                    hi[t] = hi[t].max(p[t]);
                }
            }
        }
        let centre = [0, 1, 2].map(|t| (lo[t] + hi[t]) / 2.0);
        let size = (0..3).map(|t| hi[t] - lo[t]).fold(0.0, f64::max);
        let half = if self.half_m > 0.0 { self.half_m * 1000.0 } else { (0.75 * lam_mm).max(size) };
        let scale2 = near.scale_for(1.0).powi(2) / 2.0;
        self.job = Some(Job::spawn(ctx, "near field", move |h| {
            let cells: Vec<(f64, f64, f64)> = (0..N * N)
                .into_par_iter()
                .map(|i| {
                    let (r, c) = (i / N, i % N);
                    let a = (c as f64 / (N - 1) as f64 * 2.0 - 1.0) * half;
                    let b = (1.0 - r as f64 / (N - 1) as f64 * 2.0) * half;
                    let p = [0, 1, 2].map(|t| centre[t] + u[t] * a + v[t] * b);
                    let f = near.at(p);
                    (f.e_peak().powi(2) * scale2, f.h_peak().powi(2) * scale2, near.distance(p))
                })
                .collect();
            h.send(Grid {
                key,
                centre,
                u,
                v,
                half,
                e2: cells.iter().map(|c| c.0).collect(),
                h2: cells.iter().map(|c| c.1).collect(),
                dist: cells.iter().map(|c| c.2).collect(),
                wires,
            });
        }));
    }

    fn ratios(&self, g: &Grid, lim: &Limits) -> Vec<f64> {
        let p = self.power();
        g.e2.iter().zip(&g.h2).map(|(e2, h2)| lim.ratio((e2 * p).sqrt(), (h2 * p).sqrt())).collect()
    }

    fn plot(&mut self, ui: &mut Ui, g: &Grid, lim: &Limits) {
        let ratios = self.ratios(g, lim);
        let tex_key = format!("{}|{}|{}|{:?}", g.key, self.watts, self.duty, self.standard);
        if self.tex.as_ref().is_none_or(|t| t.0 != tex_key) {
            let mut px = vec![Color32::BLACK; N * N];
            for r in 0..N {
                for c in 0..N {
                    let i = r * N + c;
                    let over = ratios[i] > 1.0;
                    let edge = [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)].iter().any(|(dr, dc)| {
                        let (rr, cc) = (r as i32 + dr, c as i32 + dc);
                        rr >= 0
                            && cc >= 0
                            && (rr as usize) < N
                            && (cc as usize) < N
                            && (ratios[rr as usize * N + cc as usize] > 1.0) != over
                    });
                    px[i] = if edge && over {
                        Color32::WHITE
                    } else {
                        colour(10.0 * ratios[i].max(1e-9).log10())
                    };
                }
            }
            let img = ColorImage::new([N, N], px);
            let tex = ui.ctx().load_texture("near-field", img, egui::TextureOptions::LINEAR);
            self.tex = Some((tex_key, tex));
        }
        let side = ui.available_width().min(460.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
        let p = ui.painter_at(rect);
        if let Some((_, tex)) = &self.tex {
            p.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let to_screen = |q: Vec3| {
            let d = [q[0] - g.centre[0], q[1] - g.centre[1], q[2] - g.centre[2]];
            let a = (d[0] * g.u[0] + d[1] * g.u[1] + d[2] * g.u[2]) / g.half;
            let b = (d[0] * g.v[0] + d[1] * g.v[1] + d[2] * g.v[2]) / g.half;
            Pos2::new(
                rect.center().x + a as f32 * side / 2.0,
                rect.center().y - b as f32 * side / 2.0,
            )
        };
        for (a, b) in &g.wires {
            p.line_segment(
                [to_screen(*a), to_screen(*b)],
                Stroke::new(1.5, Color32::from_gray(230)),
            );
        }
        let scale_m = g.half / 1000.0;
        p.text(
            rect.left_top() + Vec2::new(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("±{scale_m:.2} m · white line is the limit"),
            egui::FontId::monospace(10.0),
            Color32::WHITE,
        );
        if let Some(pos) = resp.hover_pos() {
            let c = (((pos.x - rect.left()) / side) * (N - 1) as f32).round() as usize;
            let r = (((pos.y - rect.top()) / side) * (N - 1) as f32).round() as usize;
            if r < N && c < N {
                let i = r * N + c;
                let pw = self.power();
                let (e, h) = ((g.e2[i] * pw).sqrt(), (g.h2[i] * pw).sqrt());
                resp.on_hover_text(format!(
                    "{:.1} V/m · {:.3} A/m rms\n{:.0}% of the limit\n{:.2} m from the antenna",
                    e,
                    h,
                    ratios[i] * 100.0,
                    g.dist[i] / 1000.0
                ));
            }
        }
    }

    fn summary(&self, ui: &mut Ui, g: &Grid, lim: &Limits, dbi: f64) {
        let ratios = self.ratios(g, lim);
        let mut worst: f64 = 0.0;
        let mut touches = false;
        for (i, r) in ratios.iter().enumerate() {
            if *r > 1.0 {
                worst = worst.max(g.dist[i]);
                let (row, col) = (i / N, i % N);
                touches |= row == 0 || col == 0 || row == N - 1 || col == N - 1;
            }
        }
        let eirp = self.power() * 10f64.powf(dbi / 10.0);
        let far = lim.far_distance(eirp);
        let mut items = vec![("average", format!("{:.2} W", self.power()), READOUT)];
        items.push((
            "keep clear",
            if worst == 0.0 {
                "fine as shown".into()
            } else if touches {
                format!("past {:.2} m", g.half / 1000.0)
            } else {
                format!("{:.2} m", worst / 1000.0)
            },
            if worst == 0.0 { TRACE } else { WARN },
        ));
        if let Some(d) = far {
            items.push(("far-field estimate", format!("{d:.2} m"), TRACE));
        }
        readouts(ui, &items);
        hint(
            ui,
            &format!(
                "Fields from the solved currents, scaled to the transmit power and averaged over {:.0} minutes by the duty cycle. Keep clear is the furthest point in this plane still over the limit, measured to the nearest wire; the limit is met where both E and H are under their reference levels, which is the rule for the reactive near field. The far-field estimate is the plane-wave distance at the peak gain of {dbi:.1} dBi. Past the plot means the plot is too small: widen it.",
                self.standard.averaging_min()
            ),
        );
    }
}
