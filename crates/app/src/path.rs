use crate::charts::{self, GOLD, GREEN};
use crate::map::MapView;
use crate::worker::Job;
use antenna_terrain::path::{C, fresnel_radius};
use antenna_terrain::{Analysis, Dem, Endpoint, LatLon, Profile, analyse, radio_horizon};
use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use egui_bench::prelude::*;
use std::time::{Duration, Instant};

pub struct PathTab {
    pub site: (f64, f64),
    pub site_agl: f64,
    pub target: (f64, f64),
    pub target_agl: f64,
    pub k: f64,
    pub freq: f64,
    pub gain_dbi: f64,
    pub tx_dbm: f64,
    pub far_dbi: f64,
    pub cable_db: f64,
    pub sens_dbm: f64,
    dem: Dem,
    job: Option<Job<Result<Profile, String>>>,
    profile: Option<Profile>,
    analysis: Option<(f64, Analysis)>,
    fetched_for: String,
    pending: Option<(String, Instant)>,
    error: Option<String>,
    map: MapView,
    picked: Option<(f64, f64)>,
}

impl Default for PathTab {
    fn default() -> Self {
        Self {
            site: (53.2707, -9.0568),
            site_agl: 10.0,
            target: (53.20, -9.60),
            target_agl: 20.0,
            k: 4.0 / 3.0,
            freq: 162.0,
            gain_dbi: 2.15,
            tx_dbm: 33.0,
            far_dbi: 2.0,
            cable_db: 1.0,
            sens_dbm: -107.0,
            dem: Dem::default(),
            job: None,
            profile: None,
            analysis: None,
            fetched_for: String::new(),
            pending: None,
            error: None,
            map: MapView::default(),
            picked: None,
        }
    }
}

fn ep(at: (f64, f64), h: f64) -> Endpoint {
    Endpoint { at: LatLon::new(at.0, at.1), height_agl: h }
}

impl PathTab {
    fn key(&self) -> String {
        format!(
            "{:?}|{}|{:?}|{}|{}",
            self.site, self.site_agl, self.target, self.target_agl, self.k
        )
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        let freq = self.freq;
        self.map.poll(ctx);
        let key = self.key();
        if key != self.fetched_for && self.pending.as_ref().is_none_or(|(k, _)| *k != key) {
            self.pending = Some((key, Instant::now()));
        }
        if let Some((k, at)) = self.pending.clone() {
            if at.elapsed() > Duration::from_millis(400) {
                self.pending = None;
                self.fetched_for = k;
                let (dem, a, b, kf) = (
                    self.dem.clone(),
                    ep(self.site, self.site_agl),
                    ep(self.target, self.target_agl),
                    self.k,
                );
                self.job = Some(Job::spawn(ctx, "terrain", move |h| {
                    h.send(Profile::fetch(&dem, a, b, kf, 30.0));
                }));
            } else {
                ctx.request_repaint_after(Duration::from_millis(420) - at.elapsed());
            }
        }
        let done = self.job.as_mut().map(|j| j.poll()).unwrap_or_default();
        for r in done {
            self.job = None;
            match r {
                Ok(p) => {
                    self.profile = Some(p);
                    self.error = None;
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(p) = &self.profile
            && self
                .analysis
                .as_ref()
                .is_none_or(|(f, a)| *f != freq || (a.distance - p.length()).abs() > 1e-6)
        {
            self.analysis = Some((freq, analyse(p, freq)));
        }
    }

    pub fn sidebar(&mut self, ui: &mut Ui) {
        section(ui, "antenna", "where it is mounted", |ui| {
            row(ui, "lat", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.site.0)
                        .range(-85.0..=85.0)
                        .speed(0.0005)
                        .max_decimals(5),
                );
            });
            row(ui, "lon", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.site.1)
                        .range(-180.0..=180.0)
                        .speed(0.0005)
                        .max_decimals(5),
                );
            });
            row_help(
                ui,
                "height m",
                "Above the ground under it, not above sea level. The ground comes from the terrain model.",
                |ui| {
                    ui.add(egui::DragValue::new(&mut self.site_agl).range(0.0..=500.0).speed(0.5));
                },
            );
        });
        ui.add_space(8.0);
        section(ui, "far end", "who it talks to", |ui| {
            row(ui, "lat", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.target.0)
                        .range(-85.0..=85.0)
                        .speed(0.0005)
                        .max_decimals(5),
                );
            });
            row(ui, "lon", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.target.1)
                        .range(-180.0..=180.0)
                        .speed(0.0005)
                        .max_decimals(5),
                );
            });
            row(ui, "height m", |ui| {
                ui.add(egui::DragValue::new(&mut self.target_agl).range(0.0..=500.0).speed(0.5));
            });
            row_help(
                ui,
                "k factor",
                "Effective earth radius factor. 4/3 is standard atmosphere; lower it for sub-refraction, raise it for ducting.",
                |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.k)
                            .range(0.5..=4.0)
                            .speed(0.01)
                            .max_decimals(3),
                    );
                },
            );
        });
        ui.add_space(8.0);
        section(ui, "link", "budget, one way", |ui| {
            row(ui, "freq MHz", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.freq)
                        .range(1.0..=30_000.0)
                        .speed(0.1)
                        .max_decimals(3),
                );
            });
            row_help(
                ui,
                "gain dBi",
                "Your antenna's gain toward the far end, at the takeoff angle shown below.",
                |ui| {
                    ui.add(egui::DragValue::new(&mut self.gain_dbi).range(-30.0..=40.0).speed(0.1));
                },
            );
            row(ui, "far tx dBm", |ui| {
                ui.add(egui::DragValue::new(&mut self.tx_dbm).range(-30.0..=70.0).speed(0.5));
            });
            row(ui, "far gain dBi", |ui| {
                ui.add(egui::DragValue::new(&mut self.far_dbi).range(-20.0..=40.0).speed(0.5));
            });
            row(ui, "cable dB", |ui| {
                ui.add(egui::DragValue::new(&mut self.cable_db).range(0.0..=30.0).speed(0.1));
            });
            row(ui, "rx sens dBm", |ui| {
                ui.add(egui::DragValue::new(&mut self.sens_dbm).range(-150.0..=0.0).speed(0.5));
            });
            hint(ui, "Defaults are an AIS class B transponder (2 W) into a typical receiver.");
        });
    }

    pub fn central(&mut self, ui: &mut Ui) {
        let freq = self.freq;
        let site = self.site;
        let target = self.target;
        let profile = self.profile.clone();
        let analysis = self.analysis.as_ref().map(|a| a.1.clone());
        let horizon_m =
            profile.as_ref().map(|p| radio_horizon(p.antenna_a(), self.k)).unwrap_or(0.0);
        let clear = analysis.as_ref().is_some_and(|a| a.line_of_sight);
        let drawn = self.map.show(ui, 380.0, site, |c| {
            let a = c.at(site.0, site.1);
            let b = c.at(target.0, target.1);
            let colour = if clear { GREEN } else { FAULT };
            c.p.line_segment([a, b], Stroke::new(2.0, colour));
            if horizon_m > 0.0 {
                let centre = LatLon::new(site.0, site.1);
                let ring: Vec<Pos2> = (0..=90)
                    .map(|i| {
                        let q = centre.destination(i as f64 * 4.0, horizon_m);
                        c.at(q.lat, q.lon)
                    })
                    .collect();
                c.p.extend(Shape::dashed_line(&ring, Stroke::new(1.2, READOUT), 6.0, 5.0));
            }
            if let Some(an) = &analysis {
                for o in &an.obstacles {
                    let q = LatLon::new(site.0, site.1).destination(an.bearing, o.dist);
                    c.p.circle_stroke(c.at(q.lat, q.lon), 5.0, Stroke::new(1.5, FAULT));
                }
            }
            c.p.circle_filled(a, 6.0, READOUT);
            c.p.circle_stroke(a, 9.0, Stroke::new(1.0, READOUT));
            c.p.circle_filled(b, 5.0, TRACE);
            c.p.text(
                a + Vec2::new(10.0, -10.0),
                Align2::LEFT_BOTTOM,
                "antenna",
                theme::legend_font(11.0),
                READOUT,
            );
            c.p.text(
                b + Vec2::new(10.0, -10.0),
                Align2::LEFT_BOTTOM,
                "far end",
                theme::legend_font(11.0),
                TRACE,
            );
        });
        if drawn.response.secondary_clicked() {
            self.picked = drawn.pointer;
        }
        let picked = self.picked;
        drawn.response.context_menu(|ui| {
            if let Some(ll) = picked {
                Line::new().note(format!("{:.5}, {:.5}", ll.0, ll.1)).size(10.5).show(ui);
                if ui.button(action("put the antenna here")).clicked() {
                    self.site = ll;
                    ui.close();
                }
                if ui.button(action("put the far end here")).clicked() {
                    self.target = ll;
                    ui.close();
                }
            }
        });
        Line::new()
            .note("The dashed ring is the radio horizon to sea level from the antenna.")
            .size(10.5)
            .show(ui);
        ui.add_space(8.0);

        if let Some(e) = &self.error {
            status(ui, false, e);
        }
        if self.job.is_some() {
            progress(
                ui,
                "terrain",
                0.0,
                None,
                "fetching Copernicus GLO-30 tiles, about 25 MB each the first time",
            );
        }
        let (Some(p), Some(an)) = (profile, analysis) else {
            return;
        };
        section(
            ui,
            "profile",
            "terrain with earth curvature, line of sight and 0.6 of the first Fresnel zone",
            |ui| {
                profile_plot(ui, &p, &an, freq);
            },
        );
        ui.add_space(8.0);

        let g = self.gain_dbi;
        let loss = an.fspl_db + an.diffraction_db;
        card(
            ui,
            Some(if an.line_of_sight { OK } else { FAULT }),
            |ui| {
                Line::new().legend("path").show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Line::new().note(format!("{freq} MHz")).size(10.5).elided(ui);
                });
            },
            |ui| {
                ui.horizontal(|ui| {
                    hero(ui, "distance", &format!("{:.1}", an.distance / 1000.0), "km", TRACE);
                    ui.add_space(16.0);
                    hero(ui, "path loss", &format!("{loss:.1}"), "dB", TRACE);
                    ui.add_space(16.0);
                    {
                        let rx = self.tx_dbm + self.far_dbi + g - self.cable_db - loss;
                        hero(
                            ui,
                            "received",
                            &format!("{rx:.1}"),
                            "dBm",
                            if rx > self.sens_dbm { OK } else { FAULT },
                        );
                        ui.add_space(16.0);
                        hero(
                            ui,
                            "margin",
                            &format!("{:+.1}", rx - self.sens_dbm),
                            "dB",
                            if rx > self.sens_dbm { OK } else { FAULT },
                        );
                    }
                });
                ui.add_space(6.0);
                let items = vec![
                    ("bearing", format!("{:.1}°", an.bearing), TRACE),
                    (
                        "line of sight",
                        if an.line_of_sight { "clear".into() } else { "blocked".to_string() },
                        if an.line_of_sight { OK } else { FAULT },
                    ),
                    (
                        "fresnel 0.6",
                        if an.fresnel_clear {
                            "clear".into()
                        } else {
                            format!("{:.0} m short", -an.worst_clearance)
                        },
                        if an.fresnel_clear { OK } else { WARN },
                    ),
                    ("free space", format!("{:.1} dB", an.fspl_db), TRACE),
                    ("diffraction", format!("{:.1} dB", an.diffraction_db), TRACE),
                    ("takeoff", format!("{:+.2}°", an.takeoff_deg), TRACE),
                    ("horizon", format!("{:.1} km", horizon_m / 1000.0), TRACE),
                    ("site ground", format!("{:.0} m", p.samples[0].ground), TRACE),
                ];
                readouts(ui, &items);
            },
        );
    }
}

fn profile_plot(ui: &mut Ui, p: &Profile, an: &Analysis, freq: f64) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 240.0), Sense::hover());
    let frame = ui.painter_at(rect);
    frame.rect_filled(rect, 2.0, WELL);
    let plot =
        Rect::from_min_max(rect.min + Vec2::new(52.0, 12.0), rect.max - Vec2::new(12.0, 26.0));
    let painter = frame.with_clip_rect(plot.expand2(Vec2::new(52.0, 2.0)));
    let d = p.length();
    let lam = C / (freq * 1e6);
    let tops: Vec<f64> = p.samples.iter().map(|s| s.ground + s.bulge).collect();
    let lo = tops.iter().copied().fold(f64::INFINITY, f64::min).min(0.0);
    let hi = tops
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(p.antenna_a())
        .max(p.antenna_b())
        * 1.08
        + 5.0;
    let fx = |x: f64| plot.left() + (x / d) as f32 * plot.width();
    let fy = |y: f64| plot.bottom() - ((y - lo) / (hi - lo)) as f32 * plot.height();
    for t in charts::nice_ticks(lo, hi, 5) {
        let y = fy(t);
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0, ETCH),
        );
        frame.text(
            Pos2::new(plot.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            format!("{t:.0}"),
            theme::figure(10.0),
            LEGEND,
        );
    }
    for t in charts::nice_ticks(0.0, d / 1000.0, 8) {
        let x = fx(t * 1000.0);
        frame.text(
            Pos2::new(x, plot.bottom() + 4.0),
            Align2::CENTER_TOP,
            format!("{t:.0}"),
            theme::figure(10.0),
            LEGEND,
        );
    }
    let fill = Color32::from_rgb(0x3a, 0x4a, 0x3c);
    for (i, pair) in p.samples.windows(2).enumerate() {
        let (a, b) = (&pair[0], &pair[1]);
        let q = vec![
            Pos2::new(fx(a.dist), fy(tops[i])),
            Pos2::new(fx(b.dist), fy(tops[i + 1])),
            Pos2::new(fx(b.dist), plot.bottom()),
            Pos2::new(fx(a.dist), plot.bottom()),
        ];
        painter.add(Shape::convex_polygon(q, fill, Stroke::NONE));
    }
    let ridge: Vec<Pos2> =
        p.samples.iter().zip(&tops).map(|(s, &t)| Pos2::new(fx(s.dist), fy(t))).collect();
    painter.add(Shape::line(ridge, Stroke::new(1.2, Color32::from_rgb(0x8f, 0xb0, 0x8a))));
    let sea: Vec<Pos2> = p.samples.iter().map(|s| Pos2::new(fx(s.dist), fy(s.bulge))).collect();
    painter.extend(Shape::dashed_line(&sea, Stroke::new(1.0, TRACE.gamma_multiply(0.5)), 4.0, 4.0));
    let (ha, hb) = (p.antenna_a(), p.antenna_b());
    let los_colour = if an.line_of_sight { GREEN } else { FAULT };
    painter.line_segment(
        [Pos2::new(fx(0.0), fy(ha)), Pos2::new(fx(d), fy(hb))],
        Stroke::new(1.6, los_colour),
    );
    let fres: Vec<Pos2> = p
        .samples
        .iter()
        .map(|s| {
            Pos2::new(
                fx(s.dist),
                fy(p.los_at(s.dist) - 0.6 * fresnel_radius(lam, s.dist, d - s.dist)),
            )
        })
        .collect();
    painter.extend(Shape::dashed_line(&fres, Stroke::new(1.0, GOLD), 5.0, 4.0));
    painter.line_segment(
        [Pos2::new(fx(0.0), fy(p.samples[0].ground)), Pos2::new(fx(0.0), fy(ha))],
        Stroke::new(3.0, READOUT),
    );
    let last = &p.samples[p.samples.len() - 1];
    painter.line_segment(
        [Pos2::new(fx(d), fy(last.ground)), Pos2::new(fx(d), fy(hb))],
        Stroke::new(3.0, TRACE),
    );
    for o in &an.obstacles {
        painter.circle_stroke(Pos2::new(fx(o.dist), fy(o.height)), 5.0, Stroke::new(1.5, FAULT));
    }
    if let Some(o) = an.obstacles.iter().max_by(|a, b| a.v.total_cmp(&b.v)) {
        painter.text(
            Pos2::new(fx(o.dist) + 8.0, fy(o.height) - 6.0),
            Align2::LEFT_BOTTOM,
            format!("worst edge v {:.2}", o.v),
            theme::figure(9.5),
            FAULT,
        );
    }
    frame.text(
        Pos2::new(plot.left(), rect.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        "km · metres above sea level, earth bulge included",
        theme::legend_font(10.0),
        LEGEND,
    );
    if let Some(pos) = resp.hover_pos().filter(|q| plot.contains(*q)) {
        let dist = ((pos.x - plot.left()) / plot.width()) as f64 * d;
        if let Some(s) =
            p.samples.iter().min_by(|a, b| (a.dist - dist).abs().total_cmp(&(b.dist - dist).abs()))
        {
            painter.line_segment(
                [Pos2::new(pos.x, plot.top()), Pos2::new(pos.x, plot.bottom())],
                Stroke::new(1.0, READOUT_DIM),
            );
            let clearance = p.los_at(s.dist) - s.ground - s.bulge;
            let r1 = fresnel_radius(lam, s.dist, d - s.dist);
            painter.text(
                Pos2::new(plot.left() + 6.0, plot.top() + 4.0),
                Align2::LEFT_TOP,
                format!(
                    "{:.2} km · ground {:.0} m · bulge {:.1} m · clearance {:.0} m · F1 {:.0} m",
                    s.dist / 1000.0,
                    s.ground,
                    s.bulge,
                    clearance,
                    r1
                ),
                theme::figure(10.5),
                VALUE,
            );
        }
    }
}
