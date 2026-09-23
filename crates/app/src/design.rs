use crate::charts::{self, Chart, GOLD, GREEN, Series};
use crate::view3d::{self, Quad, View, bounds_of};
use crate::worker::Job;
use crate::{drawing, rich};
use antenna_designs::export::{cut_to_dxf, dxf_filename};
use antenna_designs::matching::{MatchKind, MatchPlan, plan_match, swr_of_50};
use antenna_designs::{
    Build, Computed, ControlId, Controls, DESIGNS, Design, FREQUENCY_PRESETS, Group, Tunable,
    WIRE_PRESETS, default_controls,
};
use antenna_solver::C64;
use antenna_solver::solve::{
    Prepared, SolveResult, SweepPoint, segment_cap, sweep, sweep_cap, swr_of, tune_to_resonance,
};
use antenna_solver::units::{Unit, format_length, wavelength};
use egui::{Color32, Ui};
use egui_bench::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SWEEP_POINTS: usize = 21;

enum Msg {
    Solved(Arc<SolveResult>, Vec<Quad>),
    Sweep(Vec<SweepPoint>, bool),
    Failed(String),
}

enum TuneMsg {
    Step(f64),
    Done(Option<(f64, C64)>),
}

pub struct DesignTab {
    pub design: &'static Design,
    pub freq: f64,
    pub wire: f64,
    pub unit: Unit,
    pub span: f64,
    pub z0: f64,
    pub controls: Controls,
    pub overrides: HashMap<String, f64>,
    computed: Option<(String, Computed)>,
    pub solved: Option<Arc<SolveResult>>,
    pub solved_for: String,
    mesh: Option<Vec<Quad>>,
    pub sweep: Vec<SweepPoint>,
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
            freq: 162.0,
            wire: 2.0,
            unit: Unit::Mm,
            span: 10.0,
            z0: 50.0,
            controls: default_controls(),
            overrides: HashMap::new(),
            computed: None,
            solved: None,
            solved_for: String::new(),
            mesh: None,
            sweep: Vec::new(),
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

impl DesignTab {
    pub fn lam(&self) -> f64 {
        wavelength(self.freq)
    }

    fn fmt(&self) -> impl Fn(f64) -> String + use<> {
        let unit = self.unit;
        move |mm| format_length(mm, unit)
    }

    fn key(&self) -> String {
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

    pub fn computed(&mut self) -> &Computed {
        let key = self.key();
        if self.computed.as_ref().is_none_or(|(k, _)| *k != key) {
            let fmt = self.fmt();
            let comp =
                self.design.run(self.lam(), self.wire, &fmt, &self.overrides, &self.controls, 1.0);
            self.computed = Some((key, comp));
        }
        &self.computed.as_ref().expect("computed").1
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
                Msg::Solved(r, mesh) => {
                    self.status = format!(
                        "{} · {} · {} unknowns · {:.0} ms",
                        if matches!(
                            self.computed().output.solve,
                            antenna_solver::geometry::Geometry::Surface(_)
                        ) {
                            "surface MoM (RWG)"
                        } else if r.hybrid {
                            "wire MoM + physical optics"
                        } else {
                            "wire MoM (Galerkin)"
                        },
                        r.how.label(),
                        r.segments,
                        r.ms
                    );
                    self.solved = Some(r);
                    self.mesh = Some(mesh);
                }
                Msg::Sweep(pts, done) => {
                    self.sweep = pts;
                    self.sweeping = !done;
                }
                Msg::Failed(e) => self.status = format!("solve failed: {e}"),
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

    fn start_solve(&mut self, ctx: &egui::Context, key: String) {
        let lam = self.lam();
        let (freq, span, wire) = (self.freq, self.span / 100.0, self.wire);
        let comp = self.computed();
        let geo = comp.output.solve.clone();
        let radius = bounds_of(&comp.output.scene).radius * 1.3;
        self.solved_for = key;
        self.sweep.clear();
        self.sweeping = true;
        self.status = "solving…".into();
        self.job = Some(Job::spawn(ctx, "solve", move |h| {
            let prepared = Prepared::new(&geo, lam, wire, segment_cap());
            let r = prepared.solve(lam, true);
            let mesh =
                r.pattern.as_ref().map(|p| view3d::pattern_mesh(p, radius)).unwrap_or_default();
            let hybrid = r.hybrid;
            if !h.send(Msg::Solved(Arc::new(r), mesh)) || hybrid {
                h.send(Msg::Sweep(Vec::new(), true));
                return;
            }
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
                None => h.send(Msg::Failed("cancelled".into())),
            };
        }));
    }

    fn start_tune(&mut self, ctx: &egui::Context) {
        let design = self.design;
        let (lam, wire) = (self.lam(), self.wire);
        let overrides = self.overrides.clone();
        let controls = self.controls.clone();
        self.tune_note = None;
        self.tune = Some(Job::spawn(ctx, "tune", move |h| {
            let fmt = |mm: f64| format!("{mm}");
            let r = tune_to_resonance(
                |s| {
                    let geo = design.run(lam, wire, &fmt, &overrides, &controls, s).output.solve;
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
        let params = self.computed().params.clone();
        self.scale_params(&params, s);
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
        (!r.hybrid).then(|| plan_match(self.design.id, r.z, self.lam(), self.z0))
    }

    pub fn sidebar(&mut self, ui: &mut Ui) {
        section(ui, "design", "what to build", |ui| {
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
        });
        ui.add_space(8.0);
        section(ui, "sweep", "what the SWR plot covers", |ui| {
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
        let lam = self.lam();
        let fmt = self.fmt();
        let design = self.design;
        let solved = self.solved.clone();
        let plan = self.plan();
        let z0 = self.z0;
        let (wire, freq) = (self.wire, self.freq);
        let spec = self.computed().output.spec.clone();

        card(
            ui,
            Some(READOUT),
            |ui| {
                Line::new().legend(design.name).show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Line::new()
                        .note(format!(
                            "λ {} · λ/2 {} · wire {:.2e} λ",
                            fmt(lam),
                            fmt(lam / 2.0),
                            wire / lam
                        ))
                        .size(10.5)
                        .elided(ui);
                });
            },
            |ui| {
                ui.horizontal(|ui| match &solved {
                    Some(r) => {
                        hero(ui, "gain", &format!("{:.1}", r.dbi.unwrap_or(0.0)), "dBi", TRACE);
                        ui.add_space(20.0);
                        hero(
                            ui,
                            "swr",
                            &format!("{:.2}", swr_of(r.z, z0)),
                            &format!(": 1 at {z0} Ω"),
                            TRACE,
                        );
                        ui.add_space(20.0);
                        let mut items = vec![
                            ("impedance", fmt_z(r.z), TRACE),
                            ("freq", format!("{freq} MHz"), READOUT),
                        ];
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
                        readouts(ui, &items);
                    }
                    None => {
                        Line::new().note(&spec).show(ui);
                    }
                });
                rich::inline(ui, design.polarisation, 12.5, LEGEND);
            },
        );
        ui.add_space(8.0);
        let status_line = self.status.clone();
        let comp = self.computed();
        let scene = comp.output.scene.clone();
        view3d::show(ui, &scene, self.mesh.as_deref(), &mut self.view, 380.0, &status_line);
        view3d::controls(ui, &mut self.view);
        ui.add_space(8.0);

        section(ui, "modelled swr", &format!("{z0} Ω reference"), |ui| {
            if solved.as_ref().is_some_and(|r| r.hybrid) {
                note(
                    ui,
                    "Not plotted. The reflector is solved by physical optics, which is coupled one way: it shapes the pattern but does not react back on the feed, so a swept SWR would be the bare feed's.",
                    LEGEND,
                );
            } else if self.sweep.len() > 1 {
                swr_chart(
                    ui,
                    &self.sweep,
                    self.freq,
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

        let comp = self.computed();
        let rows = comp.output.rows.clone();
        let cut = comp.output.cut.clone();
        let diagram = comp.output.diagram.clone();
        let feed = comp.output.feed.block();
        let notes = comp.output.notes.clone();
        let params = comp.params.clone();

        section(ui, "dimensions", "cut to these", |ui| {
            for r in &rows {
                ui.horizontal(|ui| {
                    let colour = if r.total { READOUT } else { LEGEND };
                    ui.allocate_ui(egui::vec2(ui.available_width() * 0.62, 18.0), |ui| {
                        rich::inline(ui, &r.label, 12.5, colour);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        Line::new().set(&r.value).show(ui);
                    });
                });
            }
        });
        ui.add_space(8.0);

        if let (Some(plan), Some(r)) = (&plan, &solved) {
            match_panel(ui, plan, r.z, self.freq, &fmt);
            ui.add_space(8.0);
        }

        if let Some(cut) = cut {
            section(ui, "cut file", "DXF, millimetres", |ui| {
                note(ui, &cut.note, LEGEND);
                if ui.button(action("save DXF")).clicked() {
                    let text = cut_to_dxf(&cut, design.name, self.freq);
                    let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_default();
                    let path = dir.join(dxf_filename(design.name, self.freq));
                    self.note = Some(match std::fs::write(&path, text) {
                        Ok(()) => format!("wrote {}", path.display()),
                        Err(e) => format!("could not write {}: {e}", path.display()),
                    });
                }
                if let Some(n) = &self.note {
                    Line::new().note(n).size(11.0).show(ui);
                }
            });
            ui.add_space(8.0);
        }

        if !params.is_empty() {
            self.tune_panel(ui, &params);
            ui.add_space(8.0);
        }

        section(ui, "drawing", "not to scale where it says so", |ui| {
            drawing::show(ui, &diagram);
        });
        ui.add_space(8.0);
        section(ui, "feed", feed.title, |ui| {
            if let Some(d) = &feed.drawing {
                drawing::show(ui, d);
            }
            rich::prose(ui, &feed.note);
        });
        ui.add_space(8.0);
        section(ui, "notes", "", |ui| rich::prose(ui, &notes));
    }

    fn tune_panel(&mut self, ui: &mut Ui, params: &[Tunable]) {
        let ctx = ui.ctx().clone();
        let broadband = self.design.build == Build::Sheet;
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
    }
}

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
