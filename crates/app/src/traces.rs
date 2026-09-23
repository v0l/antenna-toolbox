use antenna_vna::Point;
use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use egui_bench::prelude::*;

pub const DIVS_X: usize = 10;
pub const DIVS_Y: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    LogMag,
    Swr,
    Phase,
    R,
    X,
    Z,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::LogMag, Kind::Swr, Kind::Phase, Kind::R, Kind::X, Kind::Z];

    pub fn label(self) -> &'static str {
        match self {
            Kind::LogMag => "LOGMAG",
            Kind::Swr => "SWR",
            Kind::Phase => "PHASE",
            Kind::R => "R",
            Kind::X => "X",
            Kind::Z => "|Z|",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Kind::LogMag => "dB",
            Kind::Swr => "",
            Kind::Phase => "°",
            Kind::R | Kind::X | Kind::Z => "Ω",
        }
    }

    pub fn value(self, p: &Point, z0: f64) -> f64 {
        match self {
            Kind::LogMag => 20.0 * p.s11.norm().max(1e-9).log10(),
            Kind::Swr => p.swr(),
            Kind::Phase => p.s11.arg().to_degrees(),
            Kind::R => p.z(z0).re,
            Kind::X => p.z(z0).im,
            Kind::Z => p.z(z0).norm(),
        }
    }

    fn default_scale(self) -> (f64, f64) {
        match self {
            Kind::LogMag => (5.0, -40.0),
            Kind::Swr => (0.5, 1.0),
            Kind::Phase => (45.0, -180.0),
            Kind::R => (25.0, 0.0),
            Kind::X => (25.0, -100.0),
            Kind::Z => (25.0, 0.0),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Trace {
    pub on: bool,
    pub kind: Kind,
    pub per_div: f64,
    pub bottom: f64,
    pub colour: Color32,
}

impl Trace {
    pub fn new(kind: Kind, colour: Color32, on: bool) -> Self {
        let (per_div, bottom) = kind.default_scale();
        Trace { on, kind, per_div, bottom, colour }
    }

    pub fn set_kind(&mut self, kind: Kind) {
        if kind != self.kind {
            self.kind = kind;
            (self.per_div, self.bottom) = kind.default_scale();
        }
    }

    pub fn fit(&mut self, pts: &[Point], z0: f64) {
        let vals: Vec<f64> =
            pts.iter().map(|p| self.kind.value(p, z0)).filter(|v| v.is_finite()).collect();
        let (Some(lo), Some(hi)) =
            (vals.iter().copied().reduce(f64::min), vals.iter().copied().reduce(f64::max))
        else {
            return;
        };
        let (lo, hi) = match self.kind {
            Kind::Swr => (1.0, hi.min(20.0).max(1.5)),
            _ => (lo, hi.max(lo + 1e-6)),
        };
        let floor = match self.kind {
            Kind::LogMag => 0.5,
            Kind::Swr => 0.1,
            Kind::Phase => 5.0,
            Kind::R | Kind::X | Kind::Z => 1.0,
        };
        let raw = ((hi - lo) / DIVS_Y as f64).max(floor);
        let mag = 10f64.powf(raw.log10().floor());
        let step = [1.0, 2.0, 2.5, 5.0, 10.0]
            .iter()
            .map(|m| m * mag)
            .find(|s| *s >= raw)
            .unwrap_or(10.0 * mag);
        self.per_div = step;
        self.bottom = (lo / step).floor() * step;
        if self.bottom + step * (DIVS_Y as f64) < hi {
            self.per_div = step * 2.0;
            self.bottom = (lo / self.per_div).floor() * self.per_div;
        }
    }
}

pub fn defaults() -> [Trace; 4] {
    [
        Trace::new(Kind::LogMag, Color32::from_rgb(0xE8, 0xD4, 0x4D), true),
        Trace::new(Kind::Swr, Color32::from_rgb(0x5C, 0xD0, 0xE8), true),
        Trace::new(Kind::R, Color32::from_rgb(0xD8, 0x6B, 0xD0), false),
        Trace::new(Kind::X, Color32::from_rgb(0x6F, 0xD1, 0x8A), false),
    ]
}

fn fmt(v: f64, unit: &str) -> String {
    let n = if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    if unit.is_empty() { n } else { format!("{n} {unit}") }
}

fn nearest(pts: &[Point], mhz: f64) -> Option<&Point> {
    pts.iter().min_by(|a, b| (a.freq / 1e6 - mhz).abs().total_cmp(&(b.freq / 1e6 - mhz).abs()))
}

pub struct Response {
    pub marker: Option<Option<f64>>,
}

pub fn show(
    ui: &mut Ui,
    pts: &[Point],
    traces: &[Trace],
    z0: f64,
    target: f64,
    marker: Option<f64>,
    height: f32,
) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, Color32::BLACK);
    let legend_h = 18.0 * traces.iter().filter(|t| t.on).count().max(1) as f32 + 6.0;
    let plot =
        Rect::from_min_max(rect.min + Vec2::new(12.0, legend_h), rect.max - Vec2::new(12.0, 34.0));
    let grid = Stroke::new(1.0, Color32::from_gray(46));
    for i in 0..=DIVS_X {
        let x = plot.left() + plot.width() * i as f32 / DIVS_X as f32;
        p.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())], grid);
    }
    for i in 0..=DIVS_Y {
        let y = plot.bottom() - plot.height() * i as f32 / DIVS_Y as f32;
        p.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], grid);
    }
    let (f0, f1) = match (pts.first(), pts.last()) {
        (Some(a), Some(b)) if b.freq > a.freq => (a.freq / 1e6, b.freq / 1e6),
        _ => {
            p.text(
                plot.center(),
                Align2::CENTER_CENTER,
                "no data",
                theme::legend_font(12.0),
                LEGEND,
            );
            return Response { marker: None };
        }
    };
    let fx = |mhz: f64| plot.left() + ((mhz - f0) / (f1 - f0)) as f32 * plot.width();
    let font = theme::figure(10.5);
    for (at, align) in
        [(f0, Align2::LEFT_TOP), ((f0 + f1) / 2.0, Align2::CENTER_TOP), (f1, Align2::RIGHT_TOP)]
    {
        p.text(
            Pos2::new(fx(at), plot.bottom() + 6.0),
            align,
            format!("{at:.3} MHz"),
            font.clone(),
            LEGEND,
        );
    }
    if target > f0 && target < f1 {
        p.extend(Shape::dashed_line(
            &[Pos2::new(fx(target), plot.top()), Pos2::new(fx(target), plot.bottom())],
            Stroke::new(1.0, READOUT_DIM),
            3.0,
            4.0,
        ));
    }
    let clip = p.with_clip_rect(plot.expand(1.0));
    for t in traces.iter().rev().filter(|t| t.on) {
        let fy = |v: f64| {
            plot.bottom() - ((v - t.bottom) / (t.per_div * DIVS_Y as f64)) as f32 * plot.height()
        };
        let line: Vec<Pos2> = pts
            .iter()
            .map(|q| {
                Pos2::new(
                    fx(q.freq / 1e6),
                    fy(t.kind.value(q, z0)).clamp(plot.top() - 2.0, plot.bottom() + 2.0),
                )
            })
            .collect();
        clip.add(Shape::line(line, Stroke::new(1.6, t.colour)));
        let r = fy(t.bottom).min(plot.bottom());
        p.add(Shape::convex_polygon(
            vec![
                Pos2::new(plot.left() - 10.0, r - 4.0),
                Pos2::new(plot.left() - 3.0, r),
                Pos2::new(plot.left() - 10.0, r + 4.0),
            ],
            t.colour,
            Stroke::NONE,
        ));
    }

    let mark = marker.and_then(|m| nearest(pts, m));
    if let Some(m) = mark {
        let x = fx(m.freq / 1e6);
        p.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0, VALUE.gamma_multiply(0.6)),
        );
        p.add(Shape::convex_polygon(
            vec![
                Pos2::new(x - 5.0, plot.top() - 8.0),
                Pos2::new(x + 5.0, plot.top() - 8.0),
                Pos2::new(x, plot.top() - 1.0),
            ],
            VALUE,
            Stroke::NONE,
        ));
        for t in traces.iter().filter(|t| t.on) {
            let v = t.kind.value(m, z0);
            let y = plot.bottom()
                - ((v - t.bottom) / (t.per_div * DIVS_Y as f64)) as f32 * plot.height();
            if y >= plot.top() && y <= plot.bottom() {
                p.circle_filled(Pos2::new(x, y), 3.5, t.colour);
            }
        }
    }

    let mut y = rect.top() + 4.0;
    for (i, t) in traces.iter().enumerate().filter(|(_, t)| t.on) {
        let reading = mark.map(|m| fmt(t.kind.value(m, z0), t.kind.unit())).unwrap_or_default();
        p.text(
            Pos2::new(rect.left() + 12.0, y),
            Align2::LEFT_TOP,
            format!("CH{i} {:<6} {}/  {reading}", t.kind.label(), fmt(t.per_div, t.kind.unit())),
            font.clone(),
            t.colour,
        );
        y += 18.0;
    }
    if let Some(m) = mark {
        let z = m.z(z0);
        p.text(
            Pos2::new(rect.right() - 12.0, rect.top() + 4.0),
            Align2::RIGHT_TOP,
            format!("MARKER  {:.4} MHz", m.freq / 1e6),
            font.clone(),
            VALUE,
        );
        p.text(
            Pos2::new(rect.right() - 12.0, rect.top() + 22.0),
            Align2::RIGHT_TOP,
            format!("{:.1}{:+.1}j Ω  SWR {:.2}", z.re, z.im, m.swr()),
            font.clone(),
            VALUE,
        );
    }

    if let Some(pos) = resp.hover_pos().filter(|q| plot.contains(*q))
        && let Some(h) =
            nearest(pts, f0 + ((pos.x - plot.left()) / plot.width()) as f64 * (f1 - f0))
    {
        let x = fx(h.freq / 1e6);
        p.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0, Color32::from_gray(80)),
        );
        let z = h.z(z0);
        p.text(
            Pos2::new(rect.right() - 12.0, rect.bottom() - 4.0),
            Align2::RIGHT_BOTTOM,
            format!(
                "{:.4} MHz  {:.1}{:+.1}j Ω  SWR {:.2}  click to place the marker",
                h.freq / 1e6,
                z.re,
                z.im,
                h.swr()
            ),
            font,
            LEGEND,
        );
    }
    let set = resp
        .clicked()
        .then(|| resp.interact_pointer_pos())
        .flatten()
        .filter(|q| plot.contains(*q))
        .map(|q| Some(f0 + ((q.x - plot.left()) / plot.width()) as f64 * (f1 - f0)));
    let clear = resp.secondary_clicked().then_some(None);
    Response { marker: set.or(clear) }
}

pub fn controls(ui: &mut Ui, traces: &mut [Trace], pts: &[Point], z0: f64) {
    egui::Grid::new("traces").num_columns(6).spacing([10.0, 4.0]).show(ui, |ui| {
        for (i, t) in traces.iter_mut().enumerate() {
            let label = egui::RichText::new(format!("CH{i}"))
                .font(theme::legend_font(11.0))
                .color(if t.on { t.colour } else { LEGEND });
            if ui.add(egui::Button::new(label).selected(t.on)).clicked() {
                t.on = !t.on;
            }
            let mut kind = t.kind;
            egui::ComboBox::from_id_salt(("trace-kind", i))
                .width(90.0)
                .selected_text(kind.label())
                .show_ui(ui, |ui| {
                    for k in Kind::ALL {
                        ui.selectable_value(&mut kind, k, k.label());
                    }
                });
            t.set_kind(kind);
            ui.label(legend("per div"));
            let speed = t.per_div * 0.02;
            ui.add(
                egui::DragValue::new(&mut t.per_div).range(1e-3..=1e4).speed(speed).max_decimals(3),
            );
            ui.label(legend("bottom"));
            ui.horizontal(|ui| {
                let speed = t.per_div * 0.1;
                ui.add(egui::DragValue::new(&mut t.bottom).speed(speed).max_decimals(3));
                if ui.small_button("auto").clicked() {
                    t.fit(pts, z0);
                }
            });
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use antenna_vna::C64;

    #[test]
    fn auto_scale_puts_every_point_on_the_grid() {
        let pts: Vec<Point> = (0..50)
            .map(|i| Point {
                freq: 1e8 + i as f64 * 1e6,
                s11: C64::from_polar(0.1 + i as f64 * 0.015, i as f64 * 0.1),
                s21: None,
            })
            .collect();
        for kind in Kind::ALL {
            let mut t = Trace::new(kind, Color32::WHITE, true);
            t.fit(&pts, 50.0);
            let top = t.bottom + t.per_div * DIVS_Y as f64;
            for p in &pts {
                let v = kind.value(p, 50.0);
                if kind == Kind::Swr && v > 20.0 {
                    continue;
                }
                assert!(
                    v >= t.bottom - 1e-9 && v <= top + 1e-9,
                    "{kind:?} {v} outside {} .. {top}",
                    t.bottom
                );
            }
        }
    }
}
