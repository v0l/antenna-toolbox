use antenna_solver::C64;
use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use egui_bench::prelude::*;

pub struct Series {
    pub pts: Vec<(f64, f64)>,
    pub colour: Color32,
    pub width: f32,
    pub label: String,
}

pub struct Chart {
    pub x: (f64, f64),
    pub y: (f64, f64),
    pub log_y: bool,
    pub y_ticks: Vec<f64>,
    pub x_label: String,
    pub rules: Vec<(f64, Color32)>,
    pub h_rules: Vec<(f64, Color32)>,
    pub marks: Vec<(f64, f64, Color32)>,
    pub height: f32,
}

pub fn nice_ticks(lo: f64, hi: f64, want: usize) -> Vec<f64> {
    let span = (hi - lo).abs().max(1e-12);
    let raw = span / want as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let mut t = (lo / step).ceil() * step;
    let mut out = Vec::new();
    while t <= hi + step * 1e-9 {
        out.push(t);
        t += step;
    }
    out
}

fn fmt_tick(v: f64) -> String {
    if v.abs() >= 100.0 || v.fract().abs() < 1e-9 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

impl Chart {
    pub fn show(&self, ui: &mut Ui, series: &[Series]) -> egui::Response {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, self.height), Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 2.0, WELL);
        let plot =
            Rect::from_min_max(rect.min + Vec2::new(48.0, 12.0), rect.max - Vec2::new(12.0, 30.0));
        let fy = |v: f64| {
            let t = if self.log_y {
                (v.max(self.y.0).ln() - self.y.0.ln()) / (self.y.1.ln() - self.y.0.ln())
            } else {
                (v - self.y.0) / (self.y.1 - self.y.0)
            };
            plot.bottom() - t.clamp(-0.02, 1.02) as f32 * plot.height()
        };
        let fx =
            |v: f64| plot.left() + ((v - self.x.0) / (self.x.1 - self.x.0)) as f32 * plot.width();
        let font = theme::figure(10.5);
        for &t in &self.y_ticks {
            let y = fy(t);
            p.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                Stroke::new(1.0, ETCH),
            );
            p.text(
                Pos2::new(plot.left() - 6.0, y),
                Align2::RIGHT_CENTER,
                fmt_tick(t),
                font.clone(),
                LEGEND,
            );
        }
        for &(v, c) in &self.h_rules {
            let y = fy(v);
            p.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                Stroke::new(1.0, c),
            );
        }
        for t in nice_ticks(self.x.0, self.x.1, 6) {
            let x = fx(t);
            p.line_segment(
                [Pos2::new(x, plot.bottom()), Pos2::new(x, plot.bottom() + 4.0)],
                Stroke::new(1.0, ETCH),
            );
            p.text(
                Pos2::new(x, plot.bottom() + 6.0),
                Align2::CENTER_TOP,
                fmt_tick(t),
                font.clone(),
                LEGEND,
            );
        }
        for &(v, c) in &self.rules {
            let x = fx(v);
            p.extend(Shape::dashed_line(
                &[Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                Stroke::new(1.0, c),
                3.0,
                4.0,
            ));
        }
        let clip = p.with_clip_rect(plot.expand(2.0));
        for s in series {
            let pts: Vec<Pos2> = s.pts.iter().map(|&(x, y)| Pos2::new(fx(x), fy(y))).collect();
            if pts.len() > 1 {
                clip.add(Shape::line(pts, Stroke::new(s.width, s.colour)));
            }
        }
        for &(x, y, c) in &self.marks {
            clip.circle_filled(Pos2::new(fx(x), fy(y)), 3.5, c);
        }
        p.text(
            Pos2::new(rect.left() + 8.0, rect.bottom() - 4.0),
            Align2::LEFT_BOTTOM,
            &self.x_label,
            theme::legend_font(10.0),
            LEGEND,
        );
        let mut x = plot.right() - 4.0;
        for s in series.iter().rev().filter(|s| !s.label.is_empty()) {
            let g = p.layout_no_wrap(s.label.to_uppercase(), theme::legend_font(10.0), s.colour);
            x -= g.size().x;
            p.galley(Pos2::new(x, plot.top() + 4.0), g.clone(), s.colour);
            x -= 14.0;
        }
        if let Some(pos) = resp.hover_pos().filter(|q| plot.contains(*q)) {
            let fv =
                self.x.0 + ((pos.x - plot.left()) / plot.width()) as f64 * (self.x.1 - self.x.0);
            p.line_segment(
                [Pos2::new(pos.x, plot.top()), Pos2::new(pos.x, plot.bottom())],
                Stroke::new(1.0, READOUT_DIM),
            );
            let mut text = fmt_tick((fv * 1000.0).round() / 1000.0);
            for s in series {
                if let Some(&(_, y)) =
                    s.pts.iter().min_by(|a, b| (a.0 - fv).abs().total_cmp(&(b.0 - fv).abs()))
                {
                    text.push_str(&format!("   {} {:.2}", s.label, y));
                }
            }
            p.text(
                Pos2::new(plot.left() + 6.0, plot.top() + 4.0),
                Align2::LEFT_TOP,
                text,
                font,
                VALUE,
            );
        }
        resp
    }
}

pub fn swr_axis(max: f64) -> (f64, Vec<f64>, bool) {
    for (cap, ticks) in [
        (4.0, vec![1.0, 1.5, 2.0, 3.0, 4.0]),
        (6.0, vec![1.0, 1.5, 2.0, 3.0, 4.0, 6.0]),
        (10.0, vec![1.0, 2.0, 3.0, 5.0, 10.0]),
        (20.0, vec![1.0, 2.0, 5.0, 10.0, 20.0]),
    ] {
        if max <= cap {
            return (cap, ticks, cap > 6.0);
        }
    }
    (100.0, vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0], true)
}

pub fn bandwidth(pts: &[(f64, f64)], f0: f64, z0: f64) -> String {
    let Some(best) = pts.iter().min_by(|a, b| a.1.total_cmp(&b.1)) else {
        return String::new();
    };
    let crossings: Vec<f64> = pts
        .windows(2)
        .filter(|w| (w[0].1 - 2.0) * (w[1].1 - 2.0) < 0.0)
        .map(|w| w[0].0 + (w[1].0 - w[0].0) * (2.0 - w[0].1) / (w[1].1 - w[0].1))
        .collect();
    let mhz = |v: f64| if v < 100.0 { format!("{v:.2}") } else { format!("{v:.1}") };
    if crossings.len() >= 2 {
        let width = crossings[crossings.len() - 1] - crossings[0];
        format!("{} MHz wide under 2:1 ({:.1}% of centre)", mhz(width), 100.0 * width / f0)
    } else if pts.iter().all(|p| p.1 < 2.0) {
        "under 2:1 across the whole sweep".into()
    } else if crossings.len() == 1 {
        if pts[0].1 < 2.0 {
            format!("under 2:1 from the bottom of the sweep up to {} MHz", mhz(crossings[0]))
        } else {
            format!("under 2:1 from {} MHz upward", mhz(crossings[0]))
        }
    } else {
        format!("never under 2:1 against {z0} Ω here, best {:.2}:1", best.1)
    }
}

pub fn smith(
    ui: &mut Ui,
    size: f32,
    series: &[(Vec<C64>, Color32)],
    marker: Option<(C64, Color32)>,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, WELL);
    let c = rect.center();
    let r = size / 2.0 - 12.0;
    let at = |g: C64| Pos2::new(c.x + (g.re as f32) * r, c.y - (g.im as f32) * r);
    let grid = Stroke::new(1.0, ETCH);
    p.circle_stroke(c, r, Stroke::new(1.0, LEGEND));
    p.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], grid);
    let one = C64::new(1.0, 0.0);
    for rn in [0.2, 0.5, 1.0, 2.0, 5.0] {
        let cc = rn / (1.0 + rn);
        let rad = 1.0 / (1.0 + rn);
        p.circle_stroke(at(C64::new(cc, 0.0)), rad as f32 * r, grid);
    }
    for xn in [0.2f64, 0.5, 1.0, 2.0, 5.0] {
        for sign in [1.0, -1.0] {
            let pts: Vec<Pos2> = (0..=60)
                .map(|i| {
                    let rr = (i as f64 / 60.0 * 6.0).exp() - 1.0;
                    let z = C64::new(rr, sign * xn);
                    at((z - one) / (z + one))
                })
                .collect();
            p.add(Shape::line(pts, grid));
        }
    }
    let swr2 = 1.0 / 3.0;
    p.circle_stroke(c, swr2 as f32 * r, Stroke::new(1.0, READOUT_DIM));
    for (g, col) in series {
        let pts: Vec<Pos2> = g.iter().map(|&g| at(g)).collect();
        if pts.len() > 1 {
            p.add(Shape::line(pts, Stroke::new(1.8, *col)));
        }
    }
    if let Some((g, col)) = marker {
        p.circle_filled(at(g), 4.0, col);
    }
    p.text(
        rect.left_top() + Vec2::new(6.0, 4.0),
        Align2::LEFT_TOP,
        "SMITH · ring is 2:1",
        theme::legend_font(10.0),
        LEGEND,
    );
}

pub const GREEN: Color32 = Color32::from_rgb(0x6f, 0xbf, 0x73);
pub const GOLD: Color32 = Color32::from_rgb(0xe8, 0xb2, 0x3a);
