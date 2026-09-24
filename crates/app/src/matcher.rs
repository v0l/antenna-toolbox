use crate::charts::{self, Chart, GOLD, GREEN, Series};
use crate::design::fmt_z;
use antenna_rf::cable::{CABLES, Cable};
use antenna_rf::eseries::{E12, E24, nearest};
use antenna_rf::network::{Losses, Network, Part};
use antenna_rf::synth::{self, Family, Solution};
use antenna_rf::{C64, swr};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use egui_bench::prelude::*;

type Row = (String, Network, f64, f64, Option<(f64, f64)>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Snap {
    Exact,
    E12,
    E24,
}

pub struct Matcher {
    pub families: [bool; 5],
    pub q: f64,
    pub cable: usize,
    pub snap: Snap,
    pub q_l: f64,
    pub q_c: f64,
    pub pick: usize,
}

impl Default for Matcher {
    fn default() -> Self {
        Self {
            families: [true, true, false, true, true],
            q: 3.0,
            cable: 2,
            snap: Snap::E12,
            q_l: 150.0,
            q_c: 1000.0,
            pick: 0,
        }
    }
}

const FAMILIES: [Family; 5] = [Family::L, Family::Pi, Family::T, Family::Stub, Family::Quarter];

fn fmt_l(h: f64) -> String {
    if h >= 1e-6 { format!("{:.2} µH", h * 1e6) } else { format!("{:.1} nH", h * 1e9) }
}

fn fmt_c(f: f64) -> String {
    if f >= 1e-9 { format!("{:.2} nF", f * 1e9) } else { format!("{:.2} pF", f * 1e12) }
}

fn fmt_len(m: f64) -> String {
    if m < 1.0 { format!("{:.0} mm", m * 1000.0) } else { format!("{m:.3} m") }
}

pub fn part_text(p: &Part) -> String {
    match p {
        Part::SeriesL(v) => format!("series {}", fmt_l(*v)),
        Part::SeriesC(v) => format!("series {}", fmt_c(*v)),
        Part::ShuntL(v) => format!("shunt {}", fmt_l(*v)),
        Part::ShuntC(v) => format!("shunt {}", fmt_c(*v)),
        Part::Line(r) => format!("{} of {:.0} Ω line", fmt_len(r.len_m), r.cable.z0),
        Part::OpenStub(r) => format!("open stub {}", fmt_len(r.len_m)),
        Part::ShortStub(r) => format!("shorted stub {}", fmt_len(r.len_m)),
    }
}

impl Matcher {
    fn cable(&self) -> Cable {
        CABLES[self.cable.min(CABLES.len() - 1)]
    }

    fn losses(&self) -> Losses {
        Losses { q_l: self.q_l, q_c: self.q_c }
    }

    fn built(&self, n: &Network, z0: f64) -> Network {
        let series: &[f64] = match self.snap {
            Snap::Exact => return self.real_lines(n, z0),
            Snap::E12 => &E12,
            Snap::E24 => &E24,
        };
        let parts = n
            .parts
            .iter()
            .map(|p| match *p {
                Part::SeriesL(v) => Part::SeriesL(nearest(v, series)),
                Part::SeriesC(v) => Part::SeriesC(nearest(v, series)),
                Part::ShuntL(v) => Part::ShuntL(nearest(v, series)),
                Part::ShuntC(v) => Part::ShuntC(nearest(v, series)),
                other => other,
            })
            .collect();
        self.real_lines(&Network { parts }, z0)
    }

    fn real_lines(&self, n: &Network, z0: f64) -> Network {
        let cable = self.cable();
        let swap = |mut r: antenna_rf::cable::Run| {
            if (r.cable.z0 - z0).abs() < 1e-9 && (cable.z0 - z0).abs() < 1e-9 {
                r.cable = cable;
            }
            r
        };
        Network {
            parts: n
                .parts
                .iter()
                .map(|p| match *p {
                    Part::Line(r) => Part::Line(swap(r)),
                    Part::OpenStub(r) => Part::OpenStub(swap(r)),
                    Part::ShortStub(r) => Part::ShortStub(swap(r)),
                    other => other,
                })
                .collect(),
        }
    }

    pub fn solutions(&self, z: C64, f_hz: f64, z0: f64) -> Vec<Solution> {
        synth::all(z, z0, f_hz, self.q, self.cable().vf)
            .into_iter()
            .filter(|s| FAMILIES.iter().zip(self.families).any(|(f, on)| on && *f == s.family))
            .collect()
    }

    pub fn show(&mut self, ui: &mut Ui, z: C64, f_hz: f64, z0: f64, sweep: &[(f64, C64)]) {
        let head = format!("for {} at {:.3} MHz", fmt_z(z), f_hz / 1e6);
        section(ui, "matching network", &head, |ui| {
            if z.re <= 0.0 {
                note(ui, "No resistance to match at this frequency.", LEGEND);
                return;
            }
            self.controls(ui);
            let sols = self.solutions(z, f_hz, z0);
            if sols.is_empty() {
                note(
                    ui,
                    "Nothing in the chosen families can reach this load. Lower the Q, or turn on another family.",
                    LEGEND,
                );
                return;
            }
            self.pick = self.pick.min(sols.len() - 1);
            let rows: Vec<Row> = sols
                .iter()
                .map(|s| {
                    let built = self.built(&s.net, z0);
                    let d = built.drive(z, f_hz, self.losses());
                    let band = synth::swr_band(&built, sweep, f_hz, z0, 2.0);
                    (s.note.clone(), built, swr(d.z_in, z0), d.efficiency, band)
                })
                .collect();
            for (i, (s, (_, built, w, eff, band))) in sols.iter().zip(&rows).enumerate() {
                let parts = built.parts.iter().map(part_text).collect::<Vec<_>>().join(", ");
                let text = egui::RichText::new(format!("{}  {parts}", s.family.label()))
                    .monospace()
                    .size(11.5)
                    .color(if self.pick == i { TRACE } else { VALUE });
                if ui.add(egui::Button::selectable(self.pick == i, text).wrap()).clicked() {
                    self.pick = i;
                }
                let bw = match band {
                    Some((a, b)) if sweep.len() > 1 => {
                        format!(" · SWR under 2 from {:.2} to {:.2} MHz", a / 1e6, b / 1e6)
                    }
                    _ => String::new(),
                };
                Line::new()
                    .note(format!("SWR {w:.2} · loss {:.2} dB{bw}", -10.0 * eff.max(1e-9).log10()))
                    .size(10.5)
                    .wrapped(ui);
                ui.add_space(2.0);
            }
            let (note_text, built, ..) = &rows[self.pick];
            ui.add_space(6.0);
            Line::new().note(note_text).size(10.5).wrapped(ui);
            schematic(ui, built, z0);
            if sweep.len() > 1 {
                self.chart(ui, built, f_hz, z0, sweep);
            }
            hint(
                ui,
                "Parts are listed from the antenna back toward the radio. Loss counts the coil and capacitor Q and the cable. Stub and line lengths are physical, for the cable picked above.",
            );
        });
    }

    fn controls(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            for (i, f) in FAMILIES.iter().enumerate() {
                if toggle(ui, f.label(), self.families[i]).clicked() {
                    self.families[i] = !self.families[i];
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(legend("loaded Q"));
            ui.add(egui::DragValue::new(&mut self.q).range(0.5..=50.0).speed(0.1));
            ui.label(legend("parts"));
            for (s, l) in [(Snap::Exact, "exact"), (Snap::E12, "E12"), (Snap::E24, "E24")] {
                if toggle(ui, l, self.snap == s).clicked() {
                    self.snap = s;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(legend("coil Q"));
            ui.add(egui::DragValue::new(&mut self.q_l).range(10.0..=2000.0).speed(1.0));
            ui.label(legend("cap Q"));
            ui.add(egui::DragValue::new(&mut self.q_c).range(10.0..=10000.0).speed(10.0));
            ui.label(legend("cable"));
            egui::ComboBox::from_id_salt("match-cable").selected_text(self.cable().name).show_ui(
                ui,
                |ui| {
                    for (i, c) in CABLES.iter().enumerate() {
                        ui.selectable_value(&mut self.cable, i, c.name);
                    }
                },
            );
        });
    }

    fn chart(&self, ui: &mut Ui, built: &Network, f_hz: f64, z0: f64, sweep: &[(f64, C64)]) {
        let bare: Vec<(f64, f64)> = sweep.iter().map(|(f, z)| (f / 1e6, swr(*z, z0))).collect();
        let matched: Vec<(f64, f64)> =
            sweep.iter().map(|(f, z)| (f / 1e6, swr(built.input(*z, *f), z0))).collect();
        let worst = matched.iter().map(|p| p.1).fold(1.0, f64::max).min(20.0);
        let (top, ticks, log) = charts::swr_axis(worst.max(3.0));
        let (lo, hi) = (bare[0].0, bare[bare.len() - 1].0);
        Chart {
            x: (lo, hi),
            y: (1.0, top),
            log_y: log,
            y_ticks: ticks,
            x_label: "MHz · SWR".into(),
            rules: vec![(f_hz / 1e6, Color32::from_rgb(0x4f, 0xa3, 0xc7))],
            h_rules: vec![(2.0, READOUT_DIM)],
            marks: Vec::new(),
            height: 160.0,
        }
        .show(
            ui,
            &[
                Series {
                    pts: bare,
                    colour: GOLD.gamma_multiply(0.55),
                    width: 1.4,
                    label: "bare".into(),
                },
                Series { pts: matched, colour: GREEN, width: 2.2, label: "matched".into() },
            ],
        );
    }
}

fn schematic(ui: &mut Ui, n: &Network, z0: f64) {
    let slots = n.parts.len() + 2;
    let w = ui.available_width().min(90.0 * slots as f32);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 96.0), Sense::hover());
    let p = ui.painter_at(rect);
    let stroke = Stroke::new(1.4, VALUE);
    let top = rect.top() + 26.0;
    let bottom = rect.bottom() - 18.0;
    let step = rect.width() / slots as f32;
    let x = |i: usize| rect.left() + step * (i as f32 + 0.5);
    p.line_segment([Pos2::new(x(0), top), Pos2::new(x(slots - 1), top)], stroke);
    p.line_segment([Pos2::new(x(0), bottom), Pos2::new(x(slots - 1), bottom)], stroke);
    let font = egui::FontId::monospace(10.0);
    let label = |pos: Pos2, s: &str| {
        p.text(pos, egui::Align2::CENTER_CENTER, s, font.clone(), LEGEND);
    };
    let port = |i: usize, s: &str| {
        let c = Pos2::new(x(i), (top + bottom) / 2.0);
        p.rect_filled(Rect::from_center_size(c, Vec2::new(26.0, bottom - top - 16.0)), 2.0, WELL);
        p.rect_stroke(
            Rect::from_center_size(c, Vec2::new(26.0, bottom - top - 16.0)),
            2.0,
            stroke,
            egui::StrokeKind::Middle,
        );
        label(Pos2::new(x(i), rect.top() + 9.0), s);
    };
    port(0, &format!("radio {z0:.0} Ω"));
    port(slots - 1, "antenna");
    for (k, part) in n.parts.iter().rev().enumerate() {
        let i = k + 1;
        let cx = x(i);
        let mid = (top + bottom) / 2.0;
        let (text, horizontal) = match part {
            Part::SeriesL(_) | Part::SeriesC(_) | Part::Line(_) => (part_text(part), true),
            _ => (part_text(part), false),
        };
        if horizontal {
            let a = Pos2::new(cx - step * 0.32, top);
            let b = Pos2::new(cx + step * 0.32, top);
            p.line_segment([a, b], Stroke::new(6.0, rect_bg()));
            symbol(&p, part, a, b, stroke);
            label(Pos2::new(cx, top - 14.0), text.trim_start_matches("series "));
        } else {
            let a = Pos2::new(cx, top);
            let b = Pos2::new(cx, bottom);
            p.line_segment(
                [Pos2::new(cx, top + 10.0), Pos2::new(cx, bottom - 10.0)],
                Stroke::new(6.0, rect_bg()),
            );
            p.line_segment([a, Pos2::new(cx, top + 10.0)], stroke);
            p.line_segment([Pos2::new(cx, bottom - 10.0), b], stroke);
            symbol(&p, part, Pos2::new(cx, top + 10.0), Pos2::new(cx, bottom - 10.0), stroke);
            label(Pos2::new(cx, bottom + 9.0), text.trim_start_matches("shunt "));
            let _ = mid;
        }
    }
}

fn rect_bg() -> Color32 {
    PANEL
}

fn symbol(p: &egui::Painter, part: &Part, a: Pos2, b: Pos2, stroke: Stroke) {
    let d = b - a;
    let len = d.length();
    let u = d / len;
    let n = Vec2::new(-u.y, u.x);
    let at = |t: f32, off: f32| a + u * (len * t) + n * off;
    match part {
        Part::SeriesL(_) | Part::ShuntL(_) => {
            p.line_segment([a, at(0.15, 0.0)], stroke);
            p.line_segment([at(0.85, 0.0), b], stroke);
            let turns = 4;
            let mut pts = Vec::new();
            for k in 0..=turns * 12 {
                let t = k as f32 / (turns * 12) as f32;
                let ph = t * turns as f32 * std::f32::consts::PI;
                pts.push(at(0.15 + 0.7 * t, -ph.sin().abs() * 7.0));
            }
            p.add(egui::Shape::line(pts, stroke));
        }
        Part::SeriesC(_) | Part::ShuntC(_) => {
            p.line_segment([a, at(0.45, 0.0)], stroke);
            p.line_segment([at(0.55, 0.0), b], stroke);
            p.line_segment([at(0.45, -9.0), at(0.45, 9.0)], Stroke::new(2.0, stroke.color));
            p.line_segment([at(0.55, -9.0), at(0.55, 9.0)], Stroke::new(2.0, stroke.color));
        }
        Part::Line(_) | Part::OpenStub(_) | Part::ShortStub(_) => {
            let end = if matches!(part, Part::OpenStub(_)) { 0.8 } else { 1.0 };
            p.line_segment([a, at(0.1, 0.0)], stroke);
            if !matches!(part, Part::Line(_)) {
                let r = Rect::from_two_pos(at(0.1, -4.0), at(end, 4.0));
                p.rect_stroke(r, 1.0, Stroke::new(1.2, GOLD), egui::StrokeKind::Middle);
            } else {
                p.line_segment([at(0.9, 0.0), b], stroke);
                let r = Rect::from_two_pos(at(0.1, -4.0), at(0.9, 4.0));
                p.rect_stroke(r, 1.0, Stroke::new(1.2, GOLD), egui::StrokeKind::Middle);
            }
        }
    }
}
