use crate::charts::{self, Chart, GOLD, GREEN, Series};
use crate::custom::{Custom, Ground};
use crate::view3d::{self, Quad, View, bounds_of};
use crate::worker::Job;
use crate::{drawing, rich};
use antenna_designs::export::{cut_to_dxf, dxf_filename};
use antenna_designs::matching::{MatchKind, MatchPlan, plan_match, swr_of_50};
use antenna_designs::{
    Build, Computed, ControlId, Controls, DESIGNS, Design, FREQUENCY_PRESETS, Group, Scene,
    Tunable, WIRE_PRESETS, default_controls, dress,
};
use antenna_solver::C64;
use antenna_solver::analysis::{Metrics, analyse};
use antenna_solver::geometry::{ALUMINIUM, BRASS, COPPER, Geometry, Insulation, STEEL, WireProps};
use antenna_solver::nec;
use antenna_solver::solve::{
    Prepared, SolveResult, SweepPoint, segment_cap, sweep, sweep_cap, swr_of, tune_to_resonance,
};
use antenna_solver::units::{C, Unit, format_length, wavelength};
use egui::{Color32, Ui};
use egui_bench::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use web_time::{Duration, Instant};

const SWEEP_POINTS: usize = 21;
const GAIN_POINTS: usize = 11;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Material {
    Perfect,
    Copper,
    Aluminium,
    Brass,
    Steel,
}

impl Material {
    pub const ALL: [Material; 5] = [
        Material::Copper,
        Material::Aluminium,
        Material::Brass,
        Material::Steel,
        Material::Perfect,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Material::Perfect => "perfect conductor",
            Material::Copper => "copper",
            Material::Aluminium => "aluminium",
            Material::Brass => "brass",
            Material::Steel => "steel",
        }
    }

    fn conductivity(self) -> Option<f64> {
        match self {
            Material::Perfect => None,
            Material::Copper => Some(COPPER),
            Material::Aluminium => Some(ALUMINIUM),
            Material::Brass => Some(BRASS),
            Material::Steel => Some(STEEL),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cover {
    Bare,
    Sleeve { eps: f64, tan: f64, thickness: f64 },
    Tube { eps: f64, tan: f64, inner: f64, wall: f64 },
}

pub const PLASTICS: [(&str, f64, f64); 6] = [
    ("PVC", 3.0, 0.01),
    ("polyethylene", 2.25, 0.0002),
    ("PTFE", 2.1, 0.0002),
    ("ABS", 2.9, 0.005),
    ("polycarbonate", 2.9, 0.007),
    ("fibreglass", 4.5, 0.015),
];

fn plastic_row(ui: &mut Ui, eps: &mut f64, tan: &mut f64) {
    row_help(
        ui,
        "plastic",
        "Typical VHF values. Pick the closest, then adjust εr and tan δ if you know better.",
        |ui| {
            let mut pick =
                PLASTICS.iter().position(|p| p.1 == *eps && p.2 == *tan).unwrap_or(usize::MAX);
            let mut options: Vec<(usize, String)> =
                PLASTICS.iter().enumerate().map(|(i, p)| (i, p.0.to_string())).collect();
            if pick == usize::MAX {
                options.push((usize::MAX, "custom".to_string()));
            }
            if choice(ui, "plastic", &mut pick, options) && pick < PLASTICS.len() {
                *eps = PLASTICS[pick].1;
                *tan = PLASTICS[pick].2;
            }
        },
    );
    row(ui, "εr", |ui| {
        ui.add(egui::DragValue::new(eps).range(1.0..=12.0).speed(0.05));
    });
    row(ui, "tan δ", |ui| {
        ui.add(egui::DragValue::new(tan).range(0.0..=0.2).speed(0.0005).max_decimals(4));
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Template,
    Custom,
}

enum Msg {
    Solved(Arc<SolveResult>, Vec<Quad>, Option<Metrics>),
    Sweep(Vec<SweepPoint>, bool),
    Gain(Vec<(f64, f64, f64)>),
}

enum TuneMsg {
    Step(f64),
    Done(Option<(f64, C64)>),
}

pub struct DesignTab {
    pub design: &'static Design,
    pub source: Source,
    pub custom: Custom,
    pub freq: f64,
    pub wire: f64,
    pub material: Material,
    pub cover: Cover,
    pub unit: Unit,
    pub span: f64,
    pub z0: f64,
    pub controls: Controls,
    pub overrides: HashMap<String, f64>,
    computed: Option<(String, Computed)>,
    pub solved: Option<Arc<SolveResult>>,
    pub metrics: Option<Metrics>,
    pub solved_for: String,
    mesh: Option<Vec<Quad>>,
    pub sweep: Vec<SweepPoint>,
    pub gain_sweep: Vec<(f64, f64, f64)>,
    sweeping: bool,
    status: String,
    job: Option<Job<Msg>>,
    tune: Option<Job<TuneMsg>>,
    tune_step: Option<f64>,
    tune_note: Option<String>,
    pending: Option<(String, Instant)>,
    view: View,
    edits: HashMap<String, String>,
    note: Option<String>,
    scale_key: String,
}

impl Default for DesignTab {
    fn default() -> Self {
        Self {
            design: DESIGNS[0],
            source: Source::Template,
            custom: Custom::default(),
            freq: 162.0,
            wire: 2.0,
            material: Material::Copper,
            cover: Cover::Bare,
            unit: Unit::Mm,
            span: 10.0,
            z0: 50.0,
            controls: default_controls(),
            overrides: HashMap::new(),
            computed: None,
            solved: None,
            metrics: None,
            solved_for: String::new(),
            mesh: None,
            sweep: Vec::new(),
            gain_sweep: Vec::new(),
            sweeping: false,
            status: "starting".into(),
            job: None,
            tune: None,
            tune_step: None,
            tune_note: None,
            pending: None,
            view: View::default(),
            edits: HashMap::new(),
            note: None,
            scale_key: String::new(),
        }
    }
}

pub fn fmt_z(z: C64) -> String {
    format!("{:.0}{}{:.0}j Ω", z.re, if z.im >= 0.0 { "+" } else { "−" }, z.im.abs())
}

fn scaled(c: &Custom, s: f64) -> Custom {
    let mut c = c.clone();
    for w in &mut c.wires {
        w.a = w.a.map(|v| v * s);
        w.b = w.b.map(|v| v * s);
    }
    for p in &mut c.sources {
        p.0 = p.0.map(|v| v * s);
    }
    for p in &mut c.loads {
        p.0 = p.0.map(|v| v * s);
    }
    for p in &mut c.networks {
        p.0 = p.0.map(|v| v * s);
        p.1 = p.1.map(|v| v * s);
        if let antenna_solver::geometry::Network::Line { length, .. } = &mut p.2 {
            *length *= s;
        }
    }
    c
}

impl DesignTab {
    pub fn lam(&self) -> f64 {
        wavelength(self.freq)
    }

    fn fmt(&self) -> impl Fn(f64) -> String + use<> {
        let unit = self.unit;
        move |mm| format_length(mm, unit)
    }

    pub fn props(&self) -> WireProps {
        let radius = self.wire / 2.0;
        WireProps {
            conductivity: self.material.conductivity(),
            insulation: match self.cover {
                Cover::Bare => None,
                Cover::Sleeve { eps, tan, thickness } => Some(Insulation {
                    eps_r: eps,
                    tan_d: tan,
                    inner: radius,
                    outer: radius + thickness,
                }),
                Cover::Tube { eps, tan, inner, wall } => Some(Insulation {
                    eps_r: eps,
                    tan_d: tan,
                    inner: inner.max(radius),
                    outer: inner.max(radius) + wall,
                }),
            },
        }
    }

    fn template_key(&self) -> String {
        let mut c: Vec<String> = self.controls.iter().map(|(k, v)| format!("{k:?}={v}")).collect();
        c.sort();
        let mut o: Vec<String> = self.overrides.iter().map(|(k, v)| format!("{k}={v}")).collect();
        o.sort();
        format!(
            "{}|{}|{}|{:?}|{}|{}",
            self.design.id,
            self.freq,
            self.wire,
            self.unit,
            c.join(","),
            o.join(",")
        )
    }

    fn key(&self) -> String {
        let model = match self.source {
            Source::Template => self.template_key(),
            Source::Custom => format!("custom|{}", self.custom.key()),
        };
        format!(
            "{model}|{}|{}|{:?}|{:?}|{}",
            self.freq, self.wire, self.material, self.cover, self.span
        )
    }

    pub fn computed(&mut self) -> &Computed {
        let key = self.template_key();
        if self.computed.as_ref().is_none_or(|(k, _)| *k != key) {
            let fmt = self.fmt();
            let comp =
                self.design.run(self.lam(), self.wire, &fmt, &self.overrides, &self.controls, 1.0);
            self.computed = Some((key, comp));
        }
        &self.computed.as_ref().expect("computed").1
    }

    pub fn geometry(&mut self) -> Geometry {
        let props = self.props();
        match self.source {
            Source::Template => {
                let mut g = self.computed().output.solve.clone();
                dress(&mut g, props);
                g
            }
            Source::Custom => Geometry::Wire(self.custom.geometry(props)),
        }
    }

    pub fn scene(&mut self) -> Scene {
        match self.source {
            Source::Template => self.computed().output.scene.clone(),
            Source::Custom => self.custom.scene(self.metrics.as_ref().map(|m| m.peak_dir)),
        }
    }

    pub fn name(&self) -> String {
        match self.source {
            Source::Template => self.design.name.to_string(),
            Source::Custom => self.custom.name.clone(),
        }
    }

    fn reset_scale(&mut self) {
        let key = format!("{}|{}", self.freq, self.wire);
        if key != self.scale_key {
            self.scale_key = key;
            self.overrides.clear();
            self.edits.clear();
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        self.reset_scale();
        self.dropped(ctx);
        let key = self.key();
        if self.solved_for != key && self.pending.as_ref().is_none_or(|(k, _)| *k != key) {
            self.pending = Some((key, Instant::now()));
        }
        if let Some((k, at)) = self.pending.clone() {
            if at.elapsed() >= Duration::from_millis(300) {
                self.pending = None;
                self.start_solve(ctx, k);
            } else {
                ctx.request_repaint_after(Duration::from_millis(320) - at.elapsed());
            }
        }
        let msgs = self.job.as_mut().map(|j| j.poll()).unwrap_or_default();
        for m in msgs {
            match m {
                Msg::Solved(r, mesh, metrics) => {
                    let kind = if matches!(self.geometry(), Geometry::Surface(_)) {
                        "surface MoM (RWG)"
                    } else if r.hybrid {
                        "wire MoM + physical optics"
                    } else {
                        "wire MoM (Galerkin)"
                    };
                    self.status = format!(
                        "{kind} · {} · {} unknowns · {:.0} ms",
                        r.how.label(),
                        r.segments,
                        r.ms
                    );
                    self.solved = Some(r);
                    self.mesh = Some(mesh);
                    self.metrics = metrics;
                }
                Msg::Sweep(pts, done) => {
                    self.sweep = pts;
                    self.sweeping = !done;
                }
                Msg::Gain(pts) => self.gain_sweep = pts,
            }
        }
        let tmsgs = self.tune.as_mut().map(|j| j.poll()).unwrap_or_default();
        for m in tmsgs {
            match m {
                TuneMsg::Step(s) => self.tune_step = Some(s),
                TuneMsg::Done(r) => {
                    self.tune = None;
                    self.tune_step = None;
                    self.apply_tune(r);
                }
            }
        }
    }

    fn dropped(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        for f in files {
            let text = match (&f.bytes, &f.path) {
                (Some(b), _) => String::from_utf8_lossy(b).into_owned(),
                (None, Some(p)) => match std::fs::read_to_string(p) {
                    Ok(t) => t,
                    Err(e) => {
                        self.custom.message = Some((false, format!("{}: {e}", p.display())));
                        continue;
                    }
                },
                _ => continue,
            };
            self.custom.path = f.path.map(|p| p.display().to_string()).unwrap_or(f.name);
            self.import_text(&text);
        }
    }

    pub fn import_text(&mut self, text: &str) {
        match self.custom.import(text, self.wire / 2.0) {
            Ok(freq) => {
                self.note = None;
                self.source = Source::Custom;
                if let Some(f) = freq.filter(|f| *f > 0.0) {
                    self.freq = f;
                }
                self.custom.name = std::path::Path::new(&self.custom.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "imported".into());
            }
            Err(e) => self.custom.message = Some((false, e)),
        }
    }

    fn start_solve(&mut self, ctx: &egui::Context, key: String) {
        let lam = self.lam();
        let (freq, span, wire) = (self.freq, self.span / 100.0, self.wire);
        let geo = self.geometry();
        let scene = self.scene();
        let up = scene.up();
        let radius = bounds_of(&scene).radius * 1.3;
        self.solved_for = key;
        self.sweep.clear();
        self.gain_sweep.clear();
        self.sweeping = true;
        self.status = "solving…".into();
        self.job = Some(Job::spawn(ctx, "solve", move |h| {
            let prepared = Prepared::new(&geo, lam, wire, segment_cap());
            let r = prepared.solve(lam, true);
            let mesh =
                r.pattern.as_ref().map(|p| view3d::pattern_mesh(p, radius)).unwrap_or_default();
            let metrics = analyse(&r, up);
            let boresight = metrics.as_ref().map(|m| m.peak_dir);
            let hybrid = r.hybrid;
            if !h.send(Msg::Solved(Arc::new(r), mesh, metrics)) {
                return;
            }
            if !hybrid {
                let model = Prepared::new(&geo, lam, wire, sweep_cap());
                let pts = sweep(
                    &model,
                    freq,
                    span,
                    SWEEP_POINTS,
                    |p| {
                        if p.len() > 2 && p.len() % 4 == 0 {
                            h.send(Msg::Sweep(p.to_vec(), false));
                        }
                    },
                    || h.cancelled(),
                );
                match pts {
                    Some(p) => h.send(Msg::Sweep(p, true)),
                    None => return,
                };
            } else {
                h.send(Msg::Sweep(Vec::new(), true));
            }
            let Some(dir) = boresight else {
                return;
            };
            let mut gains = Vec::new();
            for i in 0..GAIN_POINTS {
                if h.cancelled() {
                    return;
                }
                let f = freq * (1.0 - span + 2.0 * span * i as f64 / (GAIN_POINTS - 1) as f64);
                let r = prepared.solve(C / f, true);
                if let (Some(g), Some(peak)) = (r.gain_dbi(dir), r.dbi) {
                    gains.push((f, g, peak));
                }
                h.send(Msg::Gain(gains.clone()));
            }
        }));
    }

    fn start_tune(&mut self, ctx: &egui::Context) {
        let design = self.design;
        let (lam, wire, props) = (self.lam(), self.wire, self.props());
        let overrides = self.overrides.clone();
        let controls = self.controls.clone();
        let custom = (self.source == Source::Custom).then(|| self.custom.clone());
        self.tune_note = None;
        self.tune = Some(Job::spawn(ctx, "tune", move |h| {
            let fmt = |mm: f64| format!("{mm}");
            let r = tune_to_resonance(
                |s| {
                    let geo = match &custom {
                        Some(c) => Geometry::Wire(scaled(c, s).geometry(props)),
                        None => {
                            let mut g =
                                design.run(lam, wire, &fmt, &overrides, &controls, s).output.solve;
                            dress(&mut g, props);
                            g
                        }
                    };
                    Prepared::new(&geo, lam, wire, sweep_cap()).solve(lam, false).z
                },
                |s, _| {
                    h.send(TuneMsg::Step(s));
                },
            );
            h.send(TuneMsg::Done(r.map(|t| (t.scale, t.z))));
        }));
    }

    fn apply_tune(&mut self, r: Option<(f64, C64)>) {
        let Some((s, z)) = r else {
            self.tune_note = Some(
                "No resonance within ±30% of this size. That is the expected answer for a \
                 deliberately broadband shape such as a bowtie, spiral or Vivaldi: its reactance \
                 never crosses zero. For a resonant design it means a control such as spacing or \
                 angle needs changing first."
                    .into(),
            );
            return;
        };
        match self.source {
            Source::Template => {
                let params = self.computed().params.clone();
                self.scale_params(&params, s);
            }
            Source::Custom => self.custom = scaled(&self.custom, s),
        }
        self.tune_note = Some(format!(
            "Scaled every dimension by ×{s:.4} ({:+.1}%), giving {} at {} MHz.",
            (s - 1.0) * 100.0,
            fmt_z(z),
            self.freq
        ));
    }

    pub fn scale_params(&mut self, params: &[Tunable], s: f64) {
        for p in params {
            self.overrides.insert(p.key.clone(), p.val * s);
        }
        self.edits.clear();
    }

    pub fn plan(&mut self) -> Option<MatchPlan> {
        let r = self.solved.clone()?;
        let id = match self.source {
            Source::Template => self.design.id,
            Source::Custom if self.custom.ground != Ground::Free => "gp",
            Source::Custom => "dipole",
        };
        (!r.hybrid).then(|| plan_match(id, r.z, self.lam(), self.z0))
    }

    pub fn open_template_as_wires(&mut self) {
        let geo = match self.computed().output.solve.clone() {
            Geometry::Wire(w) => w,
            Geometry::Surface(_) => return,
        };
        let base = self.lam() / 2.0;
        match Custom::from_geometry(self.design.name, &geo, self.wire / 2.0, base) {
            Ok(c) => {
                self.custom = c;
                self.source = Source::Custom;
            }
            Err(e) => self.note = Some(e),
        }
    }

    pub fn pattern_snapshot(&mut self) -> Result<antenna_terrain::pattern::Pattern, String> {
        use antenna_solver::vec::{cross, dot, normalise, scale, sub};
        let r = self.solved.clone().ok_or("the design has not been solved yet")?;
        if r.pattern.is_none() {
            return Err("the design has no pattern yet".into());
        }
        let scene = self.scene();
        let u = normalise(scene.up());
        let mut f = sub(scene.beam_direction(), scale(u, dot(scene.beam_direction(), u)));
        if dot(f, f) < 1e-6 {
            let seed = if u[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
            f = sub(seed, scale(u, dot(seed, u)));
        }
        let f = normalise(f);
        let rt = cross(f, u);
        let grounded = matches!(self.geometry(), Geometry::Wire(w) if w.ground_z.is_some());
        Ok(antenna_terrain::pattern::Pattern::from_fn(
            format!("{} at {} MHz", self.name(), self.freq),
            Some(self.freq),
            true,
            grounded,
            |az, el| {
                let (a, e) = (az.to_radians(), el.to_radians());
                let d = [0, 1, 2]
                    .map(|k| e.cos() * (a.cos() * f[k] + a.sin() * rt[k]) + e.sin() * u[k]);
                r.gain_dbi(d).unwrap_or(-100.0)
            },
        ))
    }

    fn export_pattern(&mut self) {
        let p = match self.pattern_snapshot() {
            Ok(p) => p,
            Err(e) => {
                self.note = Some(e);
                return;
            }
        };
        let file = format!("{}-{}mhz.pattern", slug(&self.name()), self.freq.round());
        self.note = Some(crate::store::download(&file, &p.to_text()));
    }

    fn export_nec(&mut self) {
        let geo = match self.geometry() {
            Geometry::Wire(w) => w,
            Geometry::Surface(_) => {
                self.note = Some("sheet-metal designs have no NEC-2 wire equivalent".into());
                return;
            }
        };
        let name = self.name();
        let deck = match nec::export(&geo, self.freq, self.wire / 2.0, &name) {
            Ok(d) => d,
            Err(e) => {
                self.note = Some(e.to_string());
                return;
            }
        };
        let file = format!("{}-{}mhz.nec", slug(&name), self.freq.round());
        self.note = Some(crate::store::download(&file, &deck));
    }

    pub fn sidebar(&mut self, ui: &mut Ui) {
        section(ui, "model", "a template or your own wires", |ui| {
            ui.horizontal(|ui| {
                if toggle(ui, "template", self.source == Source::Template).clicked() {
                    self.source = Source::Template;
                }
                if toggle(ui, "custom wires", self.source == Source::Custom).clicked() {
                    self.source = Source::Custom;
                }
            });
            if self.source == Source::Template {
                for (g, label, hint) in Group::ALL {
                    Line::new().legend(label).note(format!("  {hint}")).size(10.5).elided(ui);
                    ui.horizontal_wrapped(|ui| {
                        for d in DESIGNS.iter().filter(|d| d.group == g) {
                            if toggle(ui, d.name, self.design.id == d.id)
                                .on_hover_text(d.gain)
                                .clicked()
                            {
                                self.design = d;
                                self.edits.clear();
                            }
                        }
                    });
                    ui.add_space(4.0);
                }
                let wire_only = self.design.build != Build::Sheet;
                if ui.add_enabled(wire_only, egui::Button::new(action("edit as wires"))).clicked() {
                    self.open_template_as_wires();
                }
            }
            ui.horizontal(|ui| {
                ui.label(legend("NEC file"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.custom.path)
                        .hint_text("path, or drop a .nec file")
                        .desired_width(150.0),
                );
                if ui.button(action("import")).clicked() {
                    let path = self.custom.path.clone();
                    match std::fs::read_to_string(&path) {
                        Ok(t) => self.import_text(&t),
                        Err(e) => self.custom.message = Some((false, format!("{path}: {e}"))),
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button(action("export NEC-2 deck")).clicked() {
                    self.export_nec();
                }
                if ui.button(action("export pattern")).clicked() {
                    self.export_pattern();
                }
            });
            if let Some((ok, m)) = &self.custom.message {
                status(ui, *ok, m);
            }
            if let Some(n) = &self.note {
                Line::new().note(n).size(10.5).wrapped(ui);
            }
        });
        ui.add_space(8.0);
        section(ui, "build", "frequency and material", |ui| {
            row(ui, "freq MHz", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.freq)
                        .range(1.0..=30_000.0)
                        .speed(0.1)
                        .max_decimals(3),
                );
            });
            ui.horizontal_wrapped(|ui| {
                for f in FREQUENCY_PRESETS {
                    let label = if f >= 2000.0 {
                        format!("{:.1}G", f / 1000.0)
                    } else {
                        format!("{}", f.round())
                    };
                    if ui.small_button(label).clicked() {
                        self.freq = f;
                    }
                }
            });
            row(ui, "wire mm", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.wire)
                        .range(0.05..=30.0)
                        .speed(0.01)
                        .max_decimals(2),
                );
            });
            ui.horizontal_wrapped(|ui| {
                for (name, mm) in WIRE_PRESETS {
                    if ui.small_button(format!("{name} {mm}")).clicked() {
                        self.wire = mm;
                    }
                }
            });
            row(ui, "metal", |ui| {
                choice(
                    ui,
                    "metal",
                    &mut self.material,
                    Material::ALL.map(|m| (m, m.label().to_string())),
                );
            });
            row_help(
                ui,
                "cover",
                "Insulation on the wire, or a plastic tube around it. Both pull resonance down; the tube much less than a tight sleeve.",
                |ui| {
                    let mut kind = match self.cover {
                        Cover::Bare => 0,
                        Cover::Sleeve { .. } => 1,
                        Cover::Tube { .. } => 2,
                    };
                    choice(
                        ui,
                        "cover",
                        &mut kind,
                        [
                            (0, "bare".to_string()),
                            (1, "insulated".to_string()),
                            (2, "inside a tube".to_string()),
                        ],
                    );
                    self.cover = match (kind, self.cover) {
                        (0, _) => Cover::Bare,
                        (1, c @ Cover::Sleeve { .. }) | (2, c @ Cover::Tube { .. }) => c,
                        (1, _) => Cover::Sleeve { eps: 3.0, tan: 0.01, thickness: 0.5 },
                        _ => Cover::Tube { eps: 3.0, tan: 0.01, inner: 10.0, wall: 2.0 },
                    };
                },
            );
            match &mut self.cover {
                Cover::Bare => {}
                Cover::Sleeve { eps, tan, thickness } => {
                    plastic_row(ui, eps, tan);
                    row(ui, "thick mm", |ui| {
                        ui.add(egui::DragValue::new(thickness).range(0.05..=10.0).speed(0.05));
                    });
                }
                Cover::Tube { eps, tan, inner, wall } => {
                    plastic_row(ui, eps, tan);
                    row(ui, "bore r mm", |ui| {
                        ui.add(egui::DragValue::new(inner).range(0.5..=100.0).speed(0.1));
                    });
                    row(ui, "wall mm", |ui| {
                        ui.add(egui::DragValue::new(wall).range(0.2..=20.0).speed(0.1));
                    });
                }
            }
            row(ui, "units", |ui| {
                choice(
                    ui,
                    "units",
                    &mut self.unit,
                    [
                        (Unit::Mm, "millimetres".to_string()),
                        (Unit::Cm, "centimetres".to_string()),
                        (Unit::In, "inches".to_string()),
                    ],
                );
            });
            if self.source == Source::Template {
                for &id in self.design.controls {
                    let def = id.def();
                    let v = self.controls.entry(id).or_insert(def.def);
                    if def.options.is_empty() {
                        row_help(ui, short(id), def.label, |ui| {
                            ui.add(
                                egui::DragValue::new(v)
                                    .range(def.min..=def.max)
                                    .speed(def.step)
                                    .max_decimals(2),
                            );
                        });
                    } else {
                        row(ui, short(id), |ui| {
                            let mut idx = *v as usize;
                            choice(
                                ui,
                                def.label,
                                &mut idx,
                                def.options.iter().enumerate().map(|(i, o)| (i, o.to_string())),
                            );
                            *v = idx as f64;
                        });
                    }
                    *v = id.clamp(*v);
                }
            }
        });
        ui.add_space(8.0);
        section(ui, "sweep", "what the SWR and gain plots cover", |ui| {
            row(ui, "span ±%", |ui| {
                ui.add(egui::DragValue::new(&mut self.span).range(1.0..=60.0).speed(0.5));
            });
            row(ui, "ref Ω", |ui| {
                ui.add(egui::DragValue::new(&mut self.z0).range(10.0..=600.0).speed(1.0));
            });
            ui.horizontal(|ui| {
                for z in [50.0, 75.0, 100.0, 200.0, 300.0] {
                    if ui.small_button(format!("{z} Ω")).clicked() {
                        self.z0 = z;
                    }
                }
            });
        });
    }

    pub fn central(&mut self, ui: &mut Ui) {
        let height = ui.available_height();
        ui.columns(2, |cols| {
            egui::ScrollArea::vertical().id_salt("build-pane").max_height(height).show(
                &mut cols[0],
                |ui| {
                    self.build_pane(ui);
                    ui.add_space(20.0);
                },
            );
            egui::ScrollArea::vertical().id_salt("results-pane").max_height(height).show(
                &mut cols[1],
                |ui| {
                    self.results_pane(ui);
                    ui.add_space(20.0);
                },
            );
        });
    }

    fn build_pane(&mut self, ui: &mut Ui) {
        let lam = self.lam();
        let fmt = self.fmt();
        let name = self.name();
        let wire = self.wire;
        card(
            ui,
            Some(READOUT),
            |ui| {
                Line::new().legend(&name).show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Line::new()
                        .note(format!("λ {} · wire {:.2e} λ", fmt(lam), wire / lam))
                        .size(10.5)
                        .elided(ui);
                });
            },
            |ui| match self.source {
                Source::Template => {
                    let spec = self.computed().output.spec.clone();
                    Line::new().note(spec).size(11.5).show(ui);
                    rich::inline(ui, self.design.polarisation, 12.5, LEGEND);
                }
                Source::Custom => {
                    ui.horizontal(|ui| {
                        ui.label(legend("name"));
                        ui.text_edit_singleline(&mut self.custom.name);
                    });
                }
            },
        );
        ui.add_space(8.0);
        match self.source {
            Source::Custom => {
                section(ui, "wires", "edit the model directly", |ui| self.custom.editor(ui));
                ui.add_space(8.0);
                self.tune_panel(ui, &[]);
            }
            Source::Template => {
                let comp = self.computed();
                let rows = comp.output.rows.clone();
                let cut = comp.output.cut.clone();
                let diagram = comp.output.diagram.clone();
                let feed = comp.output.feed.block();
                let notes = comp.output.notes.clone();
                let params = comp.params.clone();
                section(ui, "cut sheet", "build to these", |ui| {
                    for r in &rows {
                        ui.horizontal(|ui| {
                            let colour = if r.total { READOUT } else { LEGEND };
                            ui.allocate_ui(egui::vec2(ui.available_width() * 0.62, 18.0), |ui| {
                                rich::inline(ui, &r.label, 12.5, colour);
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    Line::new().set(&r.value).show(ui);
                                },
                            );
                        });
                    }
                });
                ui.add_space(8.0);
                section(ui, "drawing", "", |ui| drawing::show(ui, &diagram));
                ui.add_space(8.0);
                if !params.is_empty() {
                    self.tune_panel(ui, &params);
                    ui.add_space(8.0);
                }
                if let Some(cut) = cut {
                    let design = self.design;
                    section(ui, "cut file", "DXF, millimetres", |ui| {
                        note(ui, &cut.note, LEGEND);
                        if ui.button(action("save DXF")).clicked() {
                            let text = cut_to_dxf(&cut, design.name, self.freq);
                            let file = dxf_filename(design.name, self.freq);
                            self.note = Some(crate::store::download(&file, &text));
                        }
                    });
                    ui.add_space(8.0);
                }
                section(ui, "feed", feed.title, |ui| {
                    if let Some(d) = &feed.drawing {
                        drawing::show(ui, d);
                    }
                    rich::prose(ui, &feed.note);
                });
                ui.add_space(8.0);
                section(ui, "notes", "", |ui| rich::prose(ui, &notes));
            }
        }
    }

    fn results_pane(&mut self, ui: &mut Ui) {
        let solved = self.solved.clone();
        let metrics = self.metrics.clone();
        let plan = self.plan();
        let z0 = self.z0;
        let freq = self.freq;
        card(
            ui,
            Some(TRACE),
            |ui| {
                Line::new().legend("solved").show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Line::new().note(&self.status).size(10.5).elided(ui);
                });
            },
            |ui| {
                let Some(r) = &solved else {
                    note(ui, "Solving…", LEGEND);
                    return;
                };
                ui.horizontal(|ui| {
                    hero(ui, "gain", &format!("{:.1}", r.dbi.unwrap_or(0.0)), "dBi", TRACE);
                    ui.add_space(16.0);
                    hero(
                        ui,
                        "swr",
                        &format!("{:.2}", swr_of(r.z, z0)),
                        &format!(": 1 at {z0} Ω"),
                        TRACE,
                    );
                });
                ui.add_space(4.0);
                let mut items = vec![
                    ("impedance", fmt_z(r.z), TRACE),
                    ("freq", format!("{freq} MHz"), READOUT),
                ];
                if let Some(e) = r.efficiency {
                    items.push(("efficiency", format!("{:.0}%", e * 100.0), TRACE));
                }
                if let Some(m) = &metrics {
                    items.push(("front to back", format!("{:.1} dB", m.front_to_back), TRACE));
                    items.push(("peak elevation", format!("{:+.0}°", m.peak_elevation), TRACE));
                    items.push((
                        "beam az",
                        m.beamwidth_azimuth.map(|b| format!("{b:.0}°")).unwrap_or("omni".into()),
                        TRACE,
                    ));
                    items.push((
                        "beam el",
                        m.beamwidth_elevation.map(|b| format!("{b:.0}°")).unwrap_or("-".into()),
                        TRACE,
                    ));
                }
                if let Some(p) = r.pol.filter(|p| p.ar_db < 20.0) {
                    items.push((
                        "axial ratio",
                        format!("{:.1} dB {}", p.ar_db, p.hand.label()),
                        TRACE,
                    ));
                }
                if r.hybrid {
                    items.push(("note", "Z is the feed alone".into(), LEGEND));
                }
                if r.stubby > 0 {
                    items.push((
                        "unreliable",
                        format!("{} segments too fat for their length, modelled thinner", r.stubby),
                        WARN,
                    ));
                }
                readouts(ui, &items);
            },
        );
        ui.add_space(8.0);
        let status_line = self.status.clone();
        let scene = self.scene();
        view3d::show(ui, &scene, self.mesh.as_deref(), &mut self.view, 340.0, &status_line);
        view3d::controls(ui, &mut self.view);
        ui.add_space(8.0);
        if let Some(m) = &metrics {
            section(ui, "pattern cuts", "dBi, through the peak", |ui| {
                let w = (ui.available_width() / 2.0 - 6.0).max(120.0);
                ui.horizontal(|ui| {
                    charts::polar(ui, w, &m.azimuth, m.peak_dbi, "AZIMUTH", "forward");
                    charts::polar(ui, w, &m.elevation, m.peak_dbi, "ELEVATION", "up is 90°");
                });
            });
            ui.add_space(8.0);
        }
        section(ui, "modelled swr", &format!("{z0} Ω reference"), |ui| {
            if solved.as_ref().is_some_and(|r| r.hybrid) {
                note(
                    ui,
                    "Not plotted. The reflector is solved by physical optics, which is coupled one way, so a swept SWR would be the bare feed's.",
                    LEGEND,
                );
            } else if self.sweep.len() > 1 {
                swr_chart(
                    ui,
                    &self.sweep,
                    freq,
                    self.span / 100.0,
                    z0,
                    plan.as_ref(),
                    self.sweeping,
                    &[],
                );
            } else {
                note(ui, "Sweeping…", LEGEND);
            }
        });
        ui.add_space(8.0);
        if self.gain_sweep.len() > 1 {
            section(ui, "gain across the band", "toward the peak at the design frequency", |ui| {
                let span = self.span / 100.0;
                let lo = self.gain_sweep.iter().map(|p| p.1.min(p.2)).fold(f64::INFINITY, f64::min);
                let hi = self.gain_sweep.iter().map(|p| p.2).fold(f64::NEG_INFINITY, f64::max);
                let (lo, hi) = ((lo - 1.0).floor(), (hi + 1.0).ceil());
                Chart {
                    x: (freq * (1.0 - span), freq * (1.0 + span)),
                    y: (lo, hi),
                    log_y: false,
                    y_ticks: charts::nice_ticks(lo, hi, 5),
                    x_label: "MHz · dBi".into(),
                    rules: vec![(freq, Color32::from_rgb(0x4f, 0xa3, 0xc7))],
                    h_rules: Vec::new(),
                    marks: Vec::new(),
                    height: 180.0,
                }
                .show(
                    ui,
                    &[
                        Series {
                            pts: self.gain_sweep.iter().map(|p| (p.0, p.2)).collect(),
                            colour: GOLD,
                            width: 1.6,
                            label: "peak".into(),
                        },
                        Series {
                            pts: self.gain_sweep.iter().map(|p| (p.0, p.1)).collect(),
                            colour: TRACE,
                            width: 2.0,
                            label: "boresight".into(),
                        },
                    ],
                );
            });
            ui.add_space(8.0);
        }
        if let (Some(plan), Some(r)) = (&plan, &solved) {
            let fmt = self.fmt();
            match_panel(ui, plan, r.z, freq, &fmt);
        }
    }

    fn tune_panel(&mut self, ui: &mut Ui, params: &[Tunable]) {
        let ctx = ui.ctx().clone();
        let broadband = self.source == Source::Template && self.design.build == Build::Sheet;
        section(ui, "tweak and re-solve", "millimetres", |ui| {
            ui.horizontal(|ui| {
                if !broadband {
                    let label = match self.tune_step {
                        Some(s) => format!("tuning ×{s:.3}"),
                        None => "tune size to resonance".into(),
                    };
                    if ui
                        .add_enabled(self.tune.is_none(), egui::Button::new(action(label)))
                        .clicked()
                    {
                        self.start_tune(&ctx);
                    }
                }
                let dirty = params.iter().any(|p| (p.val - p.def).abs() > 1e-9);
                if dirty && ui.button(action("reset to design")).clicked() {
                    self.overrides.clear();
                    self.edits.clear();
                    self.tune_note = None;
                }
            });
            if let Some(n) = &self.tune_note {
                note(ui, n, READOUT);
            }
            hint(
                ui,
                if broadband {
                    "No tune button: this shape has no resonance to tune to. Keep the sizes and match at the feed."
                } else {
                    "Tuning scales every dimension together, holding the design's ratios, and bisects until the feed reactance crosses zero at your design frequency."
                },
            );
            egui::Grid::new("tunables").num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
                for (i, p) in params.iter().enumerate() {
                    let buf =
                        self.edits.entry(p.key.clone()).or_insert_with(|| format!("{:.2}", p.val));
                    let edited = (p.val - p.def).abs() > 1e-9;
                    ui.label(if edited { value(&p.name).color(READOUT) } else { legend(&p.name) });
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(80.0));
                    if r.lost_focus()
                        || (r.changed() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        match buf.trim().parse::<f64>() {
                            Ok(v) if v > 0.0 => {
                                self.overrides.insert(p.key.clone(), v);
                            }
                            _ => {
                                self.overrides.remove(&p.key);
                                *buf = format!("{:.2}", p.def);
                            }
                        }
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        });
    }
}

fn short(id: ControlId) -> &'static str {
    use ControlId::*;
    match id {
        Spacing => "spacing λ",
        Turns => "turns",
        Droop => "droop °",
        Spokes => "spokes",
        Angle => "angle °",
        Segments => "gores",
        FeedType => "feed",
        Cant => "cant °",
        Hand => "hand",
        Sections => "sections",
        Phasing => "phasing",
        CoilTurns => "turns",
        LoopCirc => "circ λ",
        FOverD => "f/D",
        Flare => "flare °",
        SpiralTurns => "turns",
        FeedGap => "gap %",
        Mouth => "mouth λ",
        Elements => "elements",
        Driven => "driven",
        Boom => "boom",
        BoomDia => "boom ⌀ mm",
        Height => "apex λ",
        LoopFeed => "feed",
        Reflector => "reflector",
        Tau => "τ",
        Span => "band ratio",
        ApexAngle => "apex °",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn swr_chart(
    ui: &mut Ui,
    pts: &[SweepPoint],
    f0: f64,
    span: f64,
    z0: f64,
    plan: Option<&MatchPlan>,
    partial: bool,
    extra: &[Series],
) {
    let bare: Vec<(f64, f64)> = pts.iter().map(|p| (p.f, swr_of(p.z, z0))).collect();
    let transforms = plan.filter(|m| !matches!(m.kind, MatchKind::Direct | MatchKind::Choke));
    let matched: Option<Vec<(f64, f64)>> = transforms
        .map(|m| pts.iter().map(|p| (p.f, swr_of_50(m.apply(p.z, p.f, f0), z0))).collect());
    let worst = bare.iter().map(|p| p.1).fold(1.0, f64::max);
    let cap = match &matched {
        Some(m) => worst.min(m.iter().map(|p| p.1).fold(1.0, f64::max) * 3.0),
        None => worst,
    };
    let (top, ticks, log) = charts::swr_axis(cap);
    let shown = matched.clone().unwrap_or_else(|| bare.clone());
    let best = shown.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
    let mut series = vec![Series {
        pts: bare,
        colour: if matched.is_some() { GOLD.gamma_multiply(0.55) } else { GOLD },
        width: if matched.is_some() { 1.4 } else { 2.2 },
        label: if matched.is_some() { "bare".into() } else { String::new() },
    }];
    if let Some(m) = matched {
        series.push(Series { pts: m, colour: TRACE, width: 2.2, label: "matched".into() });
    }
    for s in extra {
        series.push(Series {
            pts: s.pts.clone(),
            colour: s.colour,
            width: s.width,
            label: s.label.clone(),
        });
    }
    Chart {
        x: (f0 * (1.0 - span), f0 * (1.0 + span)),
        y: (1.0, top),
        log_y: log,
        y_ticks: ticks,
        x_label: format!("MHz{}", if log { " · log SWR" } else { "" }),
        rules: vec![(f0, Color32::from_rgb(0x4f, 0xa3, 0xc7))],
        h_rules: vec![(2.0, READOUT_DIM)],
        marks: best.map(|b| vec![(b.0, b.1, GREEN)]).unwrap_or_default(),
        height: 220.0,
    }
    .show(ui, &series);
    let mut text =
        if partial { "sweeping…".to_string() } else { charts::bandwidth(&shown, f0, z0) };
    if let Some(b) = best {
        text.push_str(&format!(" · best {:.2}:1 at {:.2} MHz", b.1, b.0));
    }
    Line::new().note(text).size(11.0).wrapped(ui);
}

fn match_panel(ui: &mut Ui, plan: &MatchPlan, z: C64, freq: f64, fmt: &dyn Fn(f64) -> String) {
    let m = plan.apply(z, freq, freq);
    section(ui, "feed and matching", plan.headline, |ui| {
        reading(ui, "antenna", format!("{} · SWR {:.2}", fmt_z(z), swr_of_50(z, 50.0)));
        if let Some(l) = &plan.line {
            reading(ui, "λ/4 ideal", format!("{:.0} Ω", l.ideal));
            reading(ui, "nearest line", format!("{} Ω, {}", l.pick.0, l.pick.1));
            reading(ui, "solid PE 0.66", fmt(l.len_vf66));
            reading(ui, "foam 0.82", fmt(l.len_vf82));
        }
        Line::new()
            .legend("into 50 Ω")
            .column(ui, 98.0)
            .measured(format!("{} · SWR {:.2}", fmt_z(m), swr_of_50(m, 50.0)))
            .show(ui);
        ui.add_space(4.0);
        note(ui, &plan.balun, LEGEND);
        if let Some(c) = plan.caveat {
            note(ui, c, READOUT);
        }
    });
}

fn slug(name: &str) -> String {
    name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}
