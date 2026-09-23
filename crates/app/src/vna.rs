use crate::charts::{self, Chart, GOLD, GREEN, Series};
use crate::design::{DesignTab, fmt_z};
use crate::worker::Job;
use antenna_solver::solve::swr_of;
use antenna_vna::{C64, Calibration, Point, Port, Standard, detect, open};
use egui::{Color32, Ui};
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Plot {
    Swr,
    ReturnLoss,
    Impedance,
    Smith,
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
    plot: Plot,
    overlay: bool,
    message: Option<(bool, String)>,
    synced: bool,
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
            plot: Plot::Swr,
            overlay: true,
            message: None,
            synced: false,
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
    pub fn live_sweep(&mut self, ctx: &egui::Context, design: &DesignTab) {
        self.centre_on(design);
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

    pub fn sidebar(&mut self, ui: &mut Ui, design: &mut DesignTab) {
        let ctx = ui.ctx().clone();
        if !self.synced {
            self.synced = true;
            self.centre_on(design);
        }
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
            if ui.button(action("centre on the design")).clicked() {
                self.centre_on(design);
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

    fn centre_on(&mut self, design: &DesignTab) {
        let span = design.freq * (design.span / 100.0).max(0.05);
        self.start = ((design.freq - span) * 1000.0).round() / 1000.0;
        self.stop = ((design.freq + span) * 1000.0).round() / 1000.0;
    }

    pub fn central(&mut self, ui: &mut Ui, design: &mut DesignTab) {
        let z0 = design.z0;
        let pts = self.measured();
        tabs(
            ui,
            &mut self.plot,
            &[
                (Plot::Swr, "swr"),
                (Plot::ReturnLoss, "return loss"),
                (Plot::Impedance, "r and x"),
                (Plot::Smith, "smith"),
            ],
        );
        if pts.is_empty() {
            section(ui, "measurement", "", |ui| {
                note(
                    ui,
                    "Connect the VNA and sweep. The modelled curve for the current design is drawn over the measurement once there is one.",
                    LEGEND,
                );
            });
            return;
        }
        let x = (pts[0].freq / 1e6, pts[pts.len() - 1].freq / 1e6);
        let model: Vec<(f64, C64)> = if self.overlay {
            design.sweep.iter().map(|p| (p.f, p.z)).filter(|p| p.0 >= x.0 && p.0 <= x.1).collect()
        } else {
            Vec::new()
        };
        let res = resonance(&pts, z0);
        let rules = vec![(design.freq, Color32::from_rgb(0x4f, 0xa3, 0xc7))];
        match self.plot {
            Plot::Swr => {
                let meas: Vec<(f64, f64)> =
                    pts.iter().map(|p| (p.freq / 1e6, swr_of(p.z(z0), z0))).collect();
                let worst = meas.iter().map(|p| p.1).fold(1.0, f64::max);
                let (top, ticks, log) = charts::swr_axis(worst);
                let mut series = vec![Series {
                    pts: meas.clone(),
                    colour: TRACE,
                    width: 2.0,
                    label: "measured".into(),
                }];
                if model.len() > 1 {
                    series.push(Series {
                        pts: model.iter().map(|&(f, z)| (f, swr_of(z, z0))).collect(),
                        colour: GOLD,
                        width: 1.6,
                        label: "modelled".into(),
                    });
                }
                Chart {
                    x,
                    y: (1.0, top),
                    log_y: log,
                    y_ticks: ticks,
                    x_label: "MHz".into(),
                    rules,
                    h_rules: vec![(2.0, READOUT_DIM)],
                    marks: res.map(|r| vec![(r.freq / 1e6, r.swr, GREEN)]).unwrap_or_default(),
                    height: 300.0,
                }
                .show(ui, &series);
                Line::new().note(charts::bandwidth(&meas, design.freq, z0)).size(11.0).show(ui);
            }
            Plot::ReturnLoss => {
                let rl: Vec<(f64, f64)> =
                    pts.iter().map(|p| (p.freq / 1e6, p.return_loss_db())).collect();
                let hi = rl.iter().map(|p| p.1).fold(10.0, f64::max).min(60.0);
                Chart {
                    x,
                    y: (0.0, hi.ceil()),
                    log_y: false,
                    y_ticks: charts::nice_ticks(0.0, hi, 6),
                    x_label: "MHz · return loss dB".into(),
                    rules,
                    h_rules: vec![(9.54, READOUT_DIM)],
                    marks: Vec::new(),
                    height: 300.0,
                }
                .show(
                    ui,
                    &[Series { pts: rl, colour: TRACE, width: 2.0, label: "measured".into() }],
                );
            }
            Plot::Impedance => {
                let r: Vec<(f64, f64)> = pts.iter().map(|p| (p.freq / 1e6, p.z(z0).re)).collect();
                let xs: Vec<(f64, f64)> = pts.iter().map(|p| (p.freq / 1e6, p.z(z0).im)).collect();
                let lo = xs.iter().map(|p| p.1).fold(0.0, f64::min).max(-500.0);
                let hi = r.iter().chain(&xs).map(|p| p.1).fold(z0, f64::max).min(1000.0);
                let mut series = vec![
                    Series { pts: r, colour: TRACE, width: 2.0, label: "R measured".into() },
                    Series {
                        pts: xs,
                        colour: TRACE.gamma_multiply(0.55),
                        width: 1.6,
                        label: "X measured".into(),
                    },
                ];
                if model.len() > 1 {
                    series.push(Series {
                        pts: model.iter().map(|&(f, z)| (f, z.re)).collect(),
                        colour: GOLD,
                        width: 1.6,
                        label: "R model".into(),
                    });
                    series.push(Series {
                        pts: model.iter().map(|&(f, z)| (f, z.im)).collect(),
                        colour: GOLD.gamma_multiply(0.55),
                        width: 1.4,
                        label: "X model".into(),
                    });
                }
                Chart {
                    x,
                    y: (lo, hi),
                    log_y: false,
                    y_ticks: charts::nice_ticks(lo, hi, 6),
                    x_label: "MHz · Ω".into(),
                    rules,
                    h_rules: vec![(0.0, LEGEND), (z0, READOUT_DIM)],
                    marks: Vec::new(),
                    height: 300.0,
                }
                .show(ui, &series);
            }
            Plot::Smith => {
                let meas: Vec<C64> = pts.iter().map(|p| p.s11).collect();
                let mut series = vec![(meas, TRACE)];
                if model.len() > 1 {
                    series.push((model.iter().map(|&(_, z)| (z - z0) / (z + z0)).collect(), GOLD));
                }
                let marker = res.map(|r| ((r.z - z0) / (r.z + z0), GREEN));
                let size = ui.available_width().min(420.0);
                charts::smith(ui, size, &series, marker);
            }
        }
        ui.horizontal(|ui| {
            if toggle(ui, "model overlay", self.overlay).clicked() {
                self.overlay = !self.overlay;
            }
            Line::new().note(format!("{} measured points", pts.len())).size(10.5).show(ui);
        });
        ui.add_space(8.0);
        self.trim(ui, design, res, &pts);
    }

    fn trim(&mut self, ui: &mut Ui, design: &mut DesignTab, res: Option<Resonance>, pts: &[Point]) {
        let z0 = design.z0;
        let f0 = design.freq;
        let at_f0 = pts
            .iter()
            .min_by(|a, b| (a.freq / 1e6 - f0).abs().total_cmp(&(b.freq / 1e6 - f0).abs()));
        card(
            ui,
            Some(READOUT),
            |ui| {
                Line::new().legend("trim").show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Line::new()
                        .note(format!("toward {f0} MHz on the {}", design.design.name))
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
                let params = design.params();
                note(
                    ui,
                    &format!(
                        "It resonates {:.1}% {} the target, so the elements are {:.1}% too {}. Scale every length by ×{ratio:.4}, which keeps the ratios the design depends on.",
                        pct.abs(),
                        if ratio > 1.0 { "above" } else { "below" },
                        pct.abs(),
                        if ratio > 1.0 { "short" } else { "long" },
                    ),
                    VALUE,
                );
                for p in params.iter().take(8) {
                    let fmt = |mm: f64| antenna_solver::units::format_length(mm, design.unit);
                    Line::new()
                        .legend(&p.name)
                        .column(ui, 150.0)
                        .value(fmt(p.val))
                        .gap(8.0)
                        .note("→")
                        .gap(8.0)
                        .set(fmt(p.val * ratio))
                        .show(ui);
                }
                if let Some(m) =
                    design.sweep.iter().min_by(|a, b| swr_of(a.z, z0).total_cmp(&swr_of(b.z, z0)))
                {
                    hint(
                        ui,
                        &format!(
                            "The model puts its best match at {:.2} MHz, so the build is {:+.1}% from the model. Anything beyond a percent or two usually means the feed leads, the choke or something nearby.",
                            m.f,
                            (fr / m.f - 1.0) * 100.0
                        ),
                    );
                }
                if ui.button(action(format!("apply ×{ratio:.4} to the design"))).clicked() {
                    design.scale_params(&params, ratio);
                }
            },
        );
    }
}
