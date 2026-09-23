use crate::charts::{self, GREEN};
use crate::design::fmt_z;
use crate::traces::{self, Trace};
use crate::worker::Job;
use antenna_solver::solve::swr_of;
use antenna_vna::{C64, Calibration, Point, Port, Standard, detect, open};
use egui::Ui;
use egui_bench::prelude::*;
use std::sync::mpsc::{Sender, channel};

enum Reply {
    Connected { describe: String, device_cal: Option<String>, max_points: usize },
    Swept(Result<Vec<Point>, String>),
    Closed(String),
}

struct Link {
    tx: Sender<(f64, f64, usize)>,
    job: Job<Reply>,
}

pub struct VnaTab {
    ports: Vec<Port>,
    selected: usize,
    link: Option<Link>,
    describe: String,
    device_cal: Option<String>,
    max_points: usize,
    start: f64,
    stop: f64,
    pub points: usize,
    continuous: bool,
    waiting: bool,
    raw: Vec<Point>,
    cal: Calibration,
    use_cal: bool,
    capture: Option<Standard>,
    traces: [Trace; 4],
    marker: Option<f64>,
    smith: bool,
    fitted: bool,
    message: Option<(bool, String)>,
    pub target: f64,
    pub z0: f64,
    pub length: f64,
}

impl Default for VnaTab {
    fn default() -> Self {
        Self {
            ports: detect(),
            selected: 0,
            link: None,
            describe: String::new(),
            device_cal: None,
            max_points: 101,
            start: 140.0,
            stop: 180.0,
            points: 401,
            continuous: false,
            waiting: false,
            raw: Vec::new(),
            cal: Calibration::default(),
            use_cal: true,
            capture: None,
            traces: traces::defaults(),
            marker: None,
            smith: true,
            fitted: false,
            message: None,
            target: 162.0,
            z0: 50.0,
            length: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Resonance {
    pub freq: f64,
    pub z: C64,
    pub swr: f64,
    pub by_reactance: bool,
}

pub fn resonance(pts: &[Point], z0: f64) -> Option<Resonance> {
    let best = pts.iter().min_by(|a, b| a.swr().total_cmp(&b.swr()))?;
    let crossing = pts
        .windows(2)
        .filter(|w| w[0].z(z0).im < 0.0 && w[1].z(z0).im >= 0.0)
        .map(|w| {
            let (x0, x1) = (w[0].z(z0).im, w[1].z(z0).im);
            let t = -x0 / (x1 - x0);
            (w[0].freq + (w[1].freq - w[0].freq) * t, w[0].z(z0) + (w[1].z(z0) - w[0].z(z0)) * t)
        })
        .min_by(|a, b| (a.0 - best.freq).abs().total_cmp(&(b.0 - best.freq).abs()));
    Some(match crossing {
        Some((f, z)) if (f - best.freq).abs() < 0.1 * best.freq => {
            Resonance { freq: f, z, swr: swr_of(z, z0), by_reactance: true }
        }
        _ => Resonance { freq: best.freq, z: best.z(z0), swr: best.swr(), by_reactance: false },
    })
}

impl VnaTab {
    fn connect(&mut self, ctx: &egui::Context) {
        let Some(port) = self.ports.get(self.selected).cloned() else {
            return;
        };
        let (tx, rx) = channel::<(f64, f64, usize)>();
        let job = Job::spawn(ctx, "vna", move |h| {
            let mut vna = match open(&port) {
                Ok(v) => v,
                Err(e) => {
                    h.send(Reply::Closed(format!("{}: {e}", port.path)));
                    return;
                }
            };
            let device_cal = vna.calibrated_on_device().then(|| vna.device_cal_status()).flatten();
            h.send(Reply::Connected {
                describe: vna.describe(),
                device_cal,
                max_points: vna.max_points(),
            });
            while let Ok((a, b, n)) = rx.recv() {
                if h.cancelled() {
                    return;
                }
                let r = vna.sweep(a * 1e6, b * 1e6, n, false).map_err(|e| e.to_string());
                let failed = r.is_err();
                if !h.send(Reply::Swept(r)) || failed {
                    h.send(Reply::Closed("device stopped answering".into()));
                    return;
                }
            }
        });
        self.link = Some(Link { tx, job });
        self.message = None;
    }

    #[cfg(test)]
    pub fn inject(&mut self, pts: Vec<Point>) {
        self.raw = pts;
    }

    #[cfg(test)]
    pub fn deliver(&mut self, pts: Vec<Point>, continuous: bool) {
        let (tx, _rx) = channel();
        let (reply_tx, reply_rx) = channel();
        let _ = reply_tx.send(Reply::Swept(Ok(pts)));
        self.continuous = continuous;
        self.link = Some(Link { tx, job: Job::from_receiver(reply_rx) });
        self.poll();
        self.link = None;
    }

    #[cfg(test)]
    pub fn trace_scales(&self) -> Vec<(f64, f64)> {
        self.traces.iter().map(|t| (t.per_div, t.bottom)).collect()
    }

    #[cfg(test)]
    pub fn live_sweep(&mut self, ctx: &egui::Context) {
        self.centre_on_target();
        self.connect(ctx);
        std::thread::sleep(std::time::Duration::from_millis(1500));
        self.poll();
        self.request();
    }

    fn request(&mut self) {
        if let Some(l) = &self.link
            && !self.waiting
            && self.stop > self.start
            && l.tx.send((self.start, self.stop, self.points.max(11))).is_ok()
        {
            self.waiting = true;
        }
    }

    pub fn poll(&mut self) {
        let msgs = self.link.as_mut().map(|l| l.job.poll()).unwrap_or_default();
        for m in msgs {
            match m {
                Reply::Connected { describe, device_cal, max_points } => {
                    self.describe = describe;
                    self.device_cal = device_cal;
                    self.max_points = max_points;
                    self.message = Some((true, "connected".into()));
                }
                Reply::Swept(Ok(pts)) => {
                    self.waiting = false;
                    let captured = self.capture.is_some();
                    if let Some(std) = self.capture.take() {
                        self.cal.store(std, &pts);
                        self.message = Some((
                            true,
                            format!("stored {} across {} points", std.label(), pts.len()),
                        ));
                    }
                    self.raw = pts;
                    if self.continuous {
                        self.request();
                    } else if !captured {
                        self.fitted = false;
                    }
                }
                Reply::Swept(Err(e)) => {
                    self.waiting = false;
                    self.message = Some((false, e));
                }
                Reply::Closed(e) => {
                    self.waiting = false;
                    self.message = Some((false, e));
                    self.link = None;
                }
            }
        }
    }

    pub fn measured(&self) -> Vec<Point> {
        if self.use_cal && self.cal.matches(&self.raw) {
            self.cal.apply(&self.raw)
        } else {
            self.raw.clone()
        }
    }

    pub fn sidebar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        section(ui, "device", "USB serial", |ui| {
            if self.ports.is_empty() {
                note(ui, "No NanoVNA found. Plug one in and rescan.", LEGEND);
            }
            for (i, p) in self.ports.iter().enumerate() {
                if toggle(ui, &format!("{} · {}", p.path, p.kind.label()), self.selected == i)
                    .clicked()
                {
                    self.selected = i;
                }
            }
            ui.horizontal(|ui| {
                if ui.button(action("rescan")).clicked() {
                    self.ports = detect();
                }
                match self.link.is_some() {
                    false => {
                        if ui
                            .add_enabled(
                                !self.ports.is_empty(),
                                egui::Button::new(action("connect")),
                            )
                            .clicked()
                        {
                            self.connect(&ctx);
                        }
                    }
                    true => {
                        if ui.button(action("disconnect")).clicked() {
                            self.link = None;
                            self.waiting = false;
                            self.continuous = false;
                        }
                    }
                }
            });
            lamp(
                ui,
                if self.link.is_some() { "linked" } else { "idle" },
                self.link.is_some(),
                false,
            );
            if !self.describe.is_empty() {
                Line::new().note(&self.describe).size(10.5).wrapped(ui);
            }
            if let Some((ok, m)) = &self.message {
                status(ui, *ok, m);
            }
        });
        ui.add_space(8.0);
        section(ui, "target", "what the antenna should do", |ui| {
            row_help(
                ui,
                "target MHz",
                "The frequency you want the antenna resonant on. Trim advice is given against this.",
                |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.target)
                            .range(0.05..=6000.0)
                            .speed(0.1)
                            .max_decimals(3),
                    );
                },
            );
            row(ui, "ref Ω", |ui| {
                ui.add(egui::DragValue::new(&mut self.z0).range(10.0..=600.0).speed(1.0));
            });
        });
        ui.add_space(8.0);
        section(ui, "sweep", "what the VNA measures", |ui| {
            row(ui, "start MHz", |ui| {
                ui.add(egui::DragValue::new(&mut self.start).range(0.05..=6000.0).speed(0.1));
            });
            row(ui, "stop MHz", |ui| {
                ui.add(egui::DragValue::new(&mut self.stop).range(0.05..=6000.0).speed(0.1));
            });
            row(ui, "points", |ui| {
                ui.add(egui::DragValue::new(&mut self.points).range(11..=4001));
            });
            if ui.button(action("centre on the target, ±10%")).clicked() {
                self.centre_on_target();
            }
            ui.horizontal(|ui| {
                let can = self.link.is_some() && !self.waiting;
                if ui.add_enabled(can, egui::Button::new(action("sweep"))).clicked() {
                    self.request();
                }
                if toggle(ui, "continuous", self.continuous).clicked() {
                    self.continuous = !self.continuous;
                    if self.continuous {
                        self.request();
                    }
                }
            });
            if self.waiting {
                progress(ui, "sweeping", 0.0, None, &format!("{} points", self.points));
            }
        });
        ui.add_space(8.0);
        section(ui, "calibration", "open, short, load at the antenna end", |ui| {
            match &self.device_cal {
                Some(s) if s.trim().is_empty() => {
                    status(ui, false, "The device reports no calibration of its own.")
                }
                Some(s) => status(ui, true, &format!("Device calibration: {s}")),
                None => status(ui, false, "This VNA returns raw data; calibrate here."),
            }
            hint(
                ui,
                "Screw each standard onto the end of the cable where the antenna goes, then capture it. The same sweep settings are used for all three and for the antenna.",
            );
            ui.horizontal(|ui| {
                for std in Standard::ALL {
                    let done = self.cal.has(std) && self.cal.freqs.len() == self.points;
                    let label =
                        if done { format!("{} ✓", std.label()) } else { std.label().to_string() };
                    let can = self.link.is_some() && !self.waiting;
                    if ui.add_enabled(can, egui::Button::new(action(label))).clicked() {
                        self.capture = Some(std);
                        self.continuous = false;
                        self.request();
                    }
                }
            });
            ui.horizontal(|ui| {
                if toggle(ui, "apply", self.use_cal).clicked() {
                    self.use_cal = !self.use_cal;
                }
                if ui.button(action("clear")).clicked() {
                    self.cal = Calibration::default();
                }
            });
            let ready = self.cal.complete() && self.cal.matches(&self.raw);
            lamp(ui, if ready { "corrected" } else { "uncorrected" }, ready && self.use_cal, false);
        });
    }

    fn centre_on_target(&mut self) {
        let span = self.target * 0.1;
        self.start = ((self.target - span) * 1000.0).round() / 1000.0;
        self.stop = ((self.target + span) * 1000.0).round() / 1000.0;
    }

    pub fn central(&mut self, ui: &mut Ui) {
        let (z0, target) = (self.z0, self.target);
        let pts = self.measured();
        if pts.is_empty() {
            section(ui, "measurement", "", |ui| {
                note(ui, "Connect the VNA and sweep.", LEGEND);
            });
            return;
        }
        if !self.fitted {
            self.fitted = true;
            for t in &mut self.traces {
                t.fit(&pts, z0);
            }
        }
        let res = resonance(&pts, z0);
        let marker = self.marker.or(res.map(|r| r.freq / 1e6));
        let height = 420.0;
        let smith_w = if self.smith { height } else { 0.0 };
        let plot_w = (ui.available_width() - smith_w - 8.0).max(200.0);
        let mut picked = None;
        ui.horizontal(|ui| {
            ui.allocate_ui(egui::vec2(plot_w, height), |ui| {
                picked = traces::show(ui, &pts, &self.traces, z0, target, marker, height).marker;
            });
            if self.smith {
                let g: Vec<C64> = pts.iter().map(|p| p.s11).collect();
                let m = marker
                    .and_then(|f| {
                        pts.iter().min_by(|a, b| {
                            (a.freq / 1e6 - f).abs().total_cmp(&(b.freq / 1e6 - f).abs())
                        })
                    })
                    .map(|p| (p.s11, GREEN));
                charts::smith(ui, height, &[(g, TRACE)], m);
            }
        });
        if let Some(m) = picked {
            self.marker = m;
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if toggle(ui, "smith", self.smith).clicked() {
                self.smith = !self.smith;
            }
            if ui.button(action("auto scale all")).clicked() {
                for t in &mut self.traces {
                    t.fit(&pts, z0);
                }
            }
            let m = if self.marker.is_some() {
                "marker placed by hand, right-click the plot to follow resonance"
            } else {
                "marker follows resonance, click the plot to place it"
            };
            Line::new().note(format!("{} points · {m}", pts.len())).size(10.5).show(ui);
        });
        traces::controls(ui, &mut self.traces, &pts, z0);
        let swr: Vec<(f64, f64)> =
            pts.iter().map(|p| (p.freq / 1e6, swr_of(p.z(z0), z0))).collect();
        Line::new().note(charts::bandwidth(&swr, target, z0)).size(11.0).show(ui);
        ui.add_space(8.0);
        self.trim(ui, res, &pts);
    }

    fn trim(&mut self, ui: &mut Ui, res: Option<Resonance>, pts: &[Point]) {
        let (z0, f0) = (self.z0, self.target);
        let at_f0 = pts
            .iter()
            .min_by(|a, b| (a.freq / 1e6 - f0).abs().total_cmp(&(b.freq / 1e6 - f0).abs()));
        card(
            ui,
            Some(READOUT),
            |ui| {
                Line::new().legend("resonance and trim").show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Line::new()
                        .note(format!("how far it is from {f0} MHz, and what to cut"))
                        .size(10.5)
                        .elided(ui);
                });
            },
            |ui| {
                let Some(r) = res else {
                    return;
                };
                let fr = r.freq / 1e6;
                ui.horizontal(|ui| {
                    hero(ui, "resonance", &format!("{fr:.2}"), "MHz", TRACE);
                    ui.add_space(20.0);
                    let mut items = vec![
                        ("there", format!("{} · SWR {:.2}", fmt_z(r.z), r.swr), TRACE),
                        (
                            "found by",
                            if r.by_reactance {
                                "X crossing zero".into()
                            } else {
                                "lowest SWR".to_string()
                            },
                            LEGEND,
                        ),
                    ];
                    if let Some(p) = at_f0 {
                        items.push((
                            "at target",
                            format!("{} · SWR {:.2}", fmt_z(p.z(z0)), p.swr()),
                            TRACE,
                        ));
                    }
                    readouts(ui, &items);
                });
                hint(
                    ui,
                    "Resonance is where the reactance crosses zero nearest the best match, or the lowest SWR when it never crosses. A resonant element's length scales inversely with frequency, so the ratio to the target is how much to cut or add.",
                );
                let ratio = fr / f0;
                let pct = (ratio - 1.0) * 100.0;
                if pct.abs() < 0.3 {
                    status(
                        ui,
                        true,
                        &format!("Resonant within {:.2}% of {f0} MHz. Leave it.", pct.abs()),
                    );
                    return;
                }
                note(
                    ui,
                    format!(
                        "It resonates {:.1}% {} the target, so the resonant lengths are {:.1}% too {}. Scale each of them by ×{ratio:.4}.",
                        pct.abs(),
                        if ratio > 1.0 { "above" } else { "below" },
                        pct.abs(),
                        if ratio > 1.0 { "short" } else { "long" },
                    ),
                    VALUE,
                );
                ui.horizontal(|ui| {
                    ui.label(legend("length now mm"));
                    ui.add(
                        egui::DragValue::new(&mut self.length).range(0.0..=100_000.0).speed(0.5),
                    );
                    if self.length > 0.0 {
                        Line::new()
                            .gap(8.0)
                            .note("cut to")
                            .gap(8.0)
                            .set(format!("{:.1} mm", self.length * ratio))
                            .show(ui);
                    }
                });
            },
        );
    }
}
