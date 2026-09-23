use crate::charts;
use antenna_terrain::pattern::{Mount, Pattern};
use egui::Ui;
use egui_bench::prelude::*;
use std::sync::Arc;

pub struct PatternSlot {
    pub pattern: Option<Arc<Pattern>>,
    pub mount: Mount,
    pub aim: bool,
    pub path: String,
    msg: Option<(bool, String)>,
    wants_design: bool,
    file: &'static str,
}

impl PatternSlot {
    pub fn new(file: &'static str) -> Self {
        Self {
            pattern: None,
            mount: Mount::default(),
            aim: false,
            path: String::new(),
            msg: None,
            wants_design: false,
            file,
        }
    }

    pub fn aimed(mut self) -> Self {
        self.aim = true;
        self
    }

    fn store(&self) -> Option<std::path::PathBuf> {
        if cfg!(test) {
            return None;
        }
        Some(dirs::config_dir()?.join("antenna-toolbox").join(self.file))
    }

    pub fn restore(&mut self) {
        if let Some(p) = self
            .store()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| Pattern::from_text(&t).ok())
        {
            self.pattern = Some(Arc::new(p));
        }
    }

    pub fn gain(&self, bearing: f64, elevation: f64, fixed: f64) -> f64 {
        match &self.pattern {
            Some(p) => {
                let mount =
                    if self.aim { Mount { heading: bearing, ..self.mount } } else { self.mount };
                p.toward(&mount, bearing, elevation) + if p.absolute { 0.0 } else { fixed }
            }
            None => fixed,
        }
    }

    pub fn key(&self) -> String {
        format!("{:?}|{:?}|{}", self.pattern.as_ref().map(Arc::as_ptr), self.mount, self.aim)
    }

    pub fn take_design_request(&mut self) -> bool {
        std::mem::take(&mut self.wants_design)
    }

    pub fn set(&mut self, p: Pattern) {
        if let Some(f) = self.store() {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(f, p.to_text());
        }
        self.msg = Some((true, format!("loaded {}", p.name)));
        self.pattern = Some(Arc::new(p));
    }

    pub fn failed(&mut self, why: String) {
        self.msg = Some((false, why));
    }

    fn clear(&mut self) {
        if let Some(f) = self.store() {
            let _ = std::fs::remove_file(f);
        }
        self.pattern = None;
        self.msg = None;
    }

    pub fn load_file(&mut self, path: &str) {
        self.path = path.to_string();
        match read_pattern(path) {
            Ok(p) => self.set(p),
            Err(e) => self.failed(e),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, id: &str, fixed: &mut f64, freq: f64, aimable: bool) {
        ui.push_id(id, |ui| {
            ui.horizontal(|ui| {
                if ui.button(action("use design")).clicked() {
                    self.wants_design = true;
                }
                if self.pattern.is_some() && ui.button(action("fixed gain")).clicked() {
                    self.clear();
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .hint_text("path to a pattern file")
                        .desired_width(ui.available_width() - 60.0),
                );
                if ui.button(action("load")).clicked() {
                    let path = self.path.trim().to_string();
                    self.load_file(&path);
                }
            });
            if let Some((ok, m)) = &self.msg {
                status(ui, *ok, m);
            }
            let absolute = self.pattern.as_ref().is_none_or(|p| p.absolute);
            row_help(
                ui,
                if absolute { "gain dBi" } else { "peak dBi" },
                "With no pattern, this gain applies in every direction. A SPLAT! pattern is only relative, so this is its peak.",
                |ui| {
                    ui.add_enabled(
                        !absolute || self.pattern.is_none(),
                        egui::DragValue::new(fixed).range(-30.0..=40.0).speed(0.1),
                    );
                },
            );
            let Some(p) = self.pattern.clone() else {
                return;
            };
            if aimable {
                row_help(
                    ui,
                    "points",
                    "A station that turns its beam toward you, or one fixed on a compass heading.",
                    |ui| {
                        choice(
                            ui,
                            "aim",
                            &mut self.aim,
                            [(true, "at the site".to_string()), (false, "on a heading".to_string())],
                        );
                    },
                );
            }
            if !self.aim {
                row_help(ui, "heading °", "Compass bearing the boresight points along.", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.mount.heading)
                            .range(0.0..=359.9)
                            .speed(1.0)
                            .suffix("°"),
                    );
                });
            }
            row_help(ui, "tilt °", "Mechanical tilt of the boresight, positive down, as in SPLAT!.", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.mount.tilt).range(-45.0..=45.0).speed(0.2).suffix("°"),
                );
            });
            row_help(
                ui,
                "roll °",
                "Turns the antenna about its boresight. 90 stands a horizontal Yagi's elements upright for vertical polarisation.",
                |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.mount.roll)
                            .range(-180.0..=180.0)
                            .speed(1.0)
                            .suffix("°"),
                    );
                },
            );
            let peak = p.peak() + if p.absolute { 0.0 } else { *fixed };
            let mount = if self.aim { Mount { heading: 0.0, ..self.mount } } else { self.mount };
            let horizon = antenna_solver::analysis::Cut {
                degrees: (0..360).map(f64::from).collect(),
                gain_dbi: (0..360)
                    .map(|d| {
                        let b = (90.0 - f64::from(d)).rem_euclid(360.0);
                        p.toward(&mount, b, 0.0) + if p.absolute { 0.0 } else { *fixed }
                    })
                    .collect(),
            };
            let w = ui.available_width().min(220.0);
            let title = if self.aim { "AT THE HORIZON, BEAM UP" } else { "AT THE HORIZON, NORTH UP" };
            charts::polar(ui, w, &horizon, peak, title, "E");
            if let Some(f) = p.freq_mhz
                && (f / freq - 1.0).abs() > 0.05
            {
                Line::new()
                    .note(format!("solved at {f} MHz, the link is at {freq} MHz"))
                    .size(10.5)
                    .wrapped(ui);
            }
            if p.mirror_below {
                hint(
                    ui,
                    "Solved over ground, so it has no pattern below the horizon; the gain just above is used for downward angles. The path model adds the ground itself, so a free-space pattern is better where you have one.",
                );
            }
        });
    }
}

pub fn read_pattern(path: &str) -> Result<Pattern, String> {
    let p = std::path::Path::new(path);
    let read = |p: &std::path::Path| {
        std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ext == "az" || ext == "el" {
        let az = p.with_extension("az");
        let el = p.with_extension("el");
        let el_text = el.exists().then(|| read(&el)).transpose()?;
        return Pattern::from_splat(&name, &read(&az)?, el_text.as_deref());
    }
    let text = read(p)?;
    if text.starts_with("# antenna-toolbox pattern") {
        Pattern::from_text(&text)
    } else if text.contains("RADIATION PATTERNS") {
        Pattern::from_nec_output(&name, &text)
    } else {
        Err(format!("{path}: not a pattern, NEC-2 output or SPLAT! file"))
    }
}
