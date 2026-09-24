use crate::charts::{Chart, Series};
use crate::worker::Job;
use antenna_solver::cma::{Mode, modes};
use antenna_solver::geometry::Geometry;
use antenna_solver::solve::{Prepared, sweep_cap};
use antenna_solver::units::C;
use antenna_solver::vec::Vec3;
use egui::{Color32, Pos2, Sense, Stroke, Ui, Vec2};
use egui_bench::prelude::*;
use std::f64::consts::PI;

const SHOWN: usize = 5;
const STEPS: usize = 15;

const COLOURS: [Color32; SHOWN] = [
    Color32::from_rgb(0x4f, 0xc3, 0xf7),
    Color32::from_rgb(0xe8, 0xb2, 0x3a),
    Color32::from_rgb(0x6f, 0xbf, 0x73),
    Color32::from_rgb(0xe5, 0x73, 0x73),
    Color32::from_rgb(0xba, 0x68, 0xc8),
];

pub struct Found {
    key: String,
    centre: Vec<Mode>,
    segs: Vec<(Vec3, Vec3, f64)>,
    currents: Vec<Vec<f64>>,
    track: Vec<Vec<(f64, f64)>>,
}

enum Msg {
    Centre(Found),
    Track(Vec<Vec<(f64, f64)>>),
    Done,
}

#[derive(Default)]
pub struct ModesPanel {
    job: Option<Job<Msg>>,
    found: Option<Found>,
    pick: usize,
}

fn segment_values(m: &antenna_solver::mom::Model, current: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; m.segs.len()];
    for (b, &c) in m.bases.iter().zip(current) {
        for h in &b.halves {
            out[h.seg] += c * 0.5 * h.sign;
        }
    }
    out
}

fn similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }
    let d: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    (d / (na * nb).max(1e-300)).abs()
}

impl ModesPanel {
    pub fn start(
        &mut self,
        ctx: &egui::Context,
        geo: Geometry,
        freq: f64,
        span: f64,
        wire: f64,
        key: String,
    ) {
        self.found = None;
        self.pick = 0;
        self.job = Some(Job::spawn(ctx, "modes", move |h| {
            let lam = C / freq;
            let Prepared::Wire(model) = Prepared::new(&geo, lam, wire, sweep_cap()) else {
                return;
            };
            let centre = modes(&model, 2.0 * PI / lam, SHOWN);
            let segs = model.segs.iter().map(|s| (s.a, s.b, s.len)).collect();
            let currents: Vec<Vec<f64>> =
                centre.iter().map(|m| segment_values(&model, &m.current)).collect();
            let shapes: Vec<Vec<f64>> = centre.iter().map(|m| m.current.clone()).collect();
            if !h.send(Msg::Centre(Found {
                key,
                centre: centre.clone(),
                segs,
                currents,
                track: Vec::new(),
            })) {
                return;
            }
            let freqs: Vec<f64> = (0..STEPS)
                .map(|i| freq * (1.0 - span + 2.0 * span * i as f64 / (STEPS - 1) as f64))
                .collect();
            let mid = STEPS / 2;
            let mut track: Vec<Vec<(f64, f64)>> = vec![vec![(0.0, f64::NAN); STEPS]; centre.len()];
            for (q, m) in centre.iter().enumerate() {
                track[q][mid] = (freqs[mid], m.significance());
            }
            for side in [(mid + 1..STEPS).collect::<Vec<_>>(), (0..mid).rev().collect()] {
                let mut prev = shapes.clone();
                for i in side {
                    if h.cancelled() {
                        return;
                    }
                    let here = modes(&model, 2.0 * PI * freqs[i] / C, SHOWN + 3);
                    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
                    for (q, shape) in prev.iter().enumerate() {
                        for (j, m) in here.iter().enumerate() {
                            pairs.push((similarity(&m.current, shape), q, j));
                        }
                    }
                    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
                    let (mut q_done, mut j_done) =
                        (vec![false; prev.len()], vec![false; here.len()]);
                    for (_, q, j) in pairs {
                        if q_done[q] || j_done[j] {
                            continue;
                        }
                        q_done[q] = true;
                        j_done[j] = true;
                        track[q][i] = (freqs[i], here[j].significance());
                        prev[q] = here[j].current.clone();
                    }
                    let shown: Vec<Vec<(f64, f64)>> = track
                        .iter()
                        .map(|t| t.iter().copied().filter(|p| p.1.is_finite()).collect())
                        .collect();
                    h.send(Msg::Track(shown));
                }
            }
            h.send(Msg::Done);
        }));
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        geo: &Geometry,
        freq: f64,
        span: f64,
        wire: f64,
        key: &str,
    ) {
        for m in self.job.as_mut().map(|j| j.poll()).unwrap_or_default() {
            match m {
                Msg::Centre(f) => self.found = Some(f),
                Msg::Track(t) => {
                    if let Some(f) = &mut self.found {
                        f.track = t;
                    }
                }
                Msg::Done => self.job = None,
            }
        }
        let ctx = ui.ctx().clone();
        section(ui, "characteristic modes", "the currents this shape supports on its own", |ui| {
            if !matches!(geo, Geometry::Wire(_)) {
                note(ui, "Modes are worked out for wire models only.", LEGEND);
                return;
            }
            let stale = self.found.as_ref().is_some_and(|f| f.key != key);
            ui.horizontal(|ui| {
                let label =
                    if self.found.is_some() && !stale { "find again" } else { "find modes" };
                if ui.button(action(label)).clicked() {
                    self.start(&ctx, geo.clone(), freq, span, wire, key.to_string());
                }
                if self.job.is_some() {
                    Line::new()
                        .note(if self.found.is_some() {
                            "tracking across the band…"
                        } else {
                            "solving…"
                        })
                        .size(10.5)
                        .show(ui);
                }
            });
            hint(
                ui,
                "Each mode is a current pattern the structure carries without any feed. Significance near 1 means it radiates well at this frequency; the feed share is how much of your feed's power lands in that mode. A mode crosses resonance where its significance peaks.",
            );
            let Some(f) = &self.found else {
                return;
            };
            if stale {
                note(ui, "The model has changed since these were found.", READOUT);
            }
            let total: f64 = f.centre.iter().map(|m| m.weight.norm_sqr()).sum::<f64>().max(1e-300);
            egui::Grid::new("modes-table").num_columns(5).spacing([14.0, 3.0]).show(ui, |ui| {
                for h in ["mode", "eigenvalue", "significance", "angle", "feed share"] {
                    ui.label(legend(h));
                }
                ui.end_row();
                for (i, m) in f.centre.iter().enumerate() {
                    let text = egui::RichText::new(format!("J{}", i + 1))
                        .color(COLOURS[i % SHOWN])
                        .monospace();
                    if ui.add(egui::Button::selectable(self.pick == i, text)).clicked() {
                        self.pick = i;
                    }
                    ui.label(value(format!("{:+.3}", m.lambda)));
                    ui.label(value(format!("{:.3}", m.significance())));
                    ui.label(value(format!("{:.0}°", m.angle_deg())));
                    ui.label(value(format!("{:.1}%", m.weight.norm_sqr() / total * 100.0)));
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            if let Some(cur) = f.currents.get(self.pick) {
                draw_currents(ui, &f.segs, cur, COLOURS[self.pick % SHOWN]);
            }
            if f.track.iter().any(|t| t.len() > 1) {
                let series: Vec<Series> = f
                    .track
                    .iter()
                    .enumerate()
                    .map(|(i, t)| Series {
                        pts: t.clone(),
                        colour: COLOURS[i % SHOWN],
                        width: if i == self.pick { 2.4 } else { 1.4 },
                        label: format!("J{}", i + 1),
                    })
                    .collect();
                Chart {
                    x: (freq * (1.0 - span), freq * (1.0 + span)),
                    y: (0.0, 1.0),
                    log_y: false,
                    y_ticks: vec![0.0, 0.25, 0.5, 0.707, 1.0],
                    x_label: "MHz · modal significance".into(),
                    rules: vec![(freq, Color32::from_rgb(0x4f, 0xa3, 0xc7))],
                    h_rules: vec![(std::f64::consts::FRAC_1_SQRT_2, READOUT_DIM)],
                    marks: Vec::new(),
                    height: 180.0,
                }
                .show(ui, &series);
            }
        });
    }
}

fn draw_currents(ui: &mut Ui, segs: &[(Vec3, Vec3, f64)], cur: &[f64], colour: Color32) {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for (a, b, _) in segs {
        for p in [a, b] {
            for t in 0..3 {
                lo[t] = lo[t].min(p[t]);
                hi[t] = hi[t].max(p[t]);
            }
        }
    }
    let span = [0, 1, 2].map(|t| hi[t] - lo[t]);
    let mut order = [0usize, 1, 2];
    order.sort_by(|a, b| span[*b].total_cmp(&span[*a]));
    let (u, v) = (order[0], order[1]);
    let w = ui.available_width();
    let h = 220.0f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, WELL);
    let scale = ((w as f64 - 24.0) / span[u].max(1e-9)).min((h as f64 - 24.0) / span[v].max(1e-9));
    let mid = [(lo[u] + hi[u]) / 2.0, (lo[v] + hi[v]) / 2.0];
    let to = |q: Vec3| {
        Pos2::new(
            rect.center().x + ((q[u] - mid[0]) * scale) as f32,
            rect.center().y - ((q[v] - mid[1]) * scale) as f32,
        )
    };
    let peak = cur.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-300);
    for ((a, b, _), c) in segs.iter().zip(cur) {
        p.line_segment([to(*a), to(*b)], Stroke::new(1.0, Color32::from_gray(70)));
        let t = (c / peak) as f32;
        let tint = if t >= 0.0 { colour } else { Color32::from_rgb(0xe5, 0x73, 0x73) };
        p.line_segment(
            [to(*a), to(*b)],
            Stroke::new(1.0 + 5.0 * t.abs(), tint.gamma_multiply(0.25 + 0.75 * t.abs())),
        );
    }
    p.text(
        rect.left_top() + Vec2::new(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        "modal current, thicker is stronger, red flows the other way",
        egui::FontId::monospace(10.0),
        LEGEND,
    );
}
