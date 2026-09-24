use crate::charts::{Chart, GOLD, GREEN, Series};
use crate::worker::Job;
use antenna_hf::p372::{Coefficients, ManMade};
use antenna_hf::p533::{Context, Deciles, Input, IonMaps, Location, Output, Pattern, run};
use egui::{Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2};
use egui_bench::prelude::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub const BANDS: [(f64, &str); 10] = [
    (1.9, "160"),
    (3.6, "80"),
    (5.35, "60"),
    (7.1, "40"),
    (10.12, "30"),
    (14.15, "20"),
    (18.1, "17"),
    (21.2, "15"),
    (24.94, "12"),
    (28.5, "10"),
];

const D2R: f64 = 0.0174532925;

type Maps = Arc<Mutex<HashMap<usize, Result<Arc<IonMaps>, String>>>>;

fn store() -> &'static Maps {
    static S: OnceLock<Maps> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn deciles() -> &'static Deciles {
    static D: OnceLock<Deciles> = OnceLock::new();
    D.get_or_init(Deciles::default)
}

pub enum Ready {
    Yes(Arc<IonMaps>),
    Waiting,
    Failed(String),
}

fn fetch(month: usize) {
    let name = antenna_hf::ionos_file(month);
    let url = format!("{}/{name}", antenna_hf::IONOS_URL);
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("antenna-toolbox/hf");
        let file = dir.join(&name);
        let bytes = std::fs::read(&file).or_else(|_| {
            ureq::get(&url)
                .call()
                .map_err(|e| e.to_string())
                .and_then(|mut r| {
                    r.body_mut()
                        .with_config()
                        .limit(64 * 1024 * 1024)
                        .read_to_vec()
                        .map_err(|e| e.to_string())
                })
                .inspect(|b| {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(&file, b);
                })
        });
        let res = bytes.and_then(|b| IonMaps::from_bin(&b)).map(Arc::new);
        if let Ok(mut m) = store().lock() {
            m.insert(month, res);
        }
    });
    #[cfg(target_arch = "wasm32")]
    ehttp::fetch(ehttp::Request::get(url), move |res| {
        let res = match res {
            Ok(r) if r.ok => IonMaps::from_bin(&r.bytes).map(Arc::new),
            Ok(r) => Err(format!("{name}: HTTP {}", r.status)),
            Err(e) => Err(e),
        };
        if let Ok(mut m) = store().lock() {
            m.insert(month, res);
        }
    });
}

pub fn maps(month: usize) -> Ready {
    let Ok(mut m) = store().lock() else {
        return Ready::Waiting;
    };
    match m.get(&month) {
        Some(Ok(v)) => Ready::Yes(v.clone()),
        Some(Err(e)) => {
            let e = e.clone();
            m.remove(&month);
            if let Ok(mut f) = FLYING.lock() {
                f.remove(&month);
            }
            Ready::Failed(e)
        }
        None => {
            drop(m);
            if FLYING.lock().is_ok_and(|mut f| f.insert(month)) {
                fetch(month);
            }
            Ready::Waiting
        }
    }
}

static FLYING: Mutex<std::collections::BTreeSet<usize>> =
    Mutex::new(std::collections::BTreeSet::new());

pub fn this_month() -> usize {
    let secs = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64 + 719_468;
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (m - 1) as usize
}

#[derive(Clone)]
pub struct Setup {
    pub month: usize,
    pub ssn: i32,
    pub watts: f64,
    pub bw: f64,
    pub snr: f64,
    pub noise: ManMade,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            month: this_month(),
            ssn: 100,
            watts: 100.0,
            bw: 3000.0,
            snr: 15.0,
            noise: ManMade::Rural,
        }
    }
}

impl Setup {
    fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{:?}",
            self.month, self.ssn, self.watts, self.bw, self.snr, self.noise
        )
    }

    fn input(&self, tx: (f64, f64), rx: (f64, f64), hour: usize, f: f64) -> Input {
        Input {
            month: self.month,
            hour,
            ssn: self.ssn,
            frequency: f,
            bw: self.bw,
            txpower: 10.0 * (self.watts / 1000.0).log10(),
            snrr: self.snr,
            snrxxp: 90,
            tx: Location { lat: tx.0 * D2R, lng: tx.1 * D2R },
            rx: Location { lat: rx.0 * D2R, lng: rx.1 * D2R },
            long_path: false,
            man_made: self.noise,
        }
    }
}

pub struct Table {
    key: String,
    cells: Vec<Output>,
}

pub struct Area {
    pub key: String,
    pub hour: usize,
    pub freq: f64,
    bcr: Vec<f32>,
    tex: Option<TextureHandle>,
}

const AREA_W: usize = 180;
const AREA_H: usize = 120;
const MERC_LAT: f64 = 80.0;

fn merc_y(lat: f64) -> f64 {
    (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln()
}

fn row_lat(r: usize) -> f64 {
    let (top, bottom) = (merc_y(MERC_LAT), merc_y(-MERC_LAT));
    let y = top + (bottom - top) * (r as f64 + 0.5) / AREA_H as f64;
    (2.0 * y.exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees()
}

fn reliability_colour(bcr: f64) -> Color32 {
    if !bcr.is_finite() || bcr < 5.0 {
        return Color32::TRANSPARENT;
    }
    let t = (bcr / 100.0).clamp(0.0, 1.0) as f32;
    let lerp = |a: u8, b: u8, t: f32| (a as f32 + (b as f32 - a as f32) * t) as u8;
    if t < 0.5 {
        let u = t / 0.5;
        Color32::from_rgb(lerp(0x2c, 0xe8, u), lerp(0x3e, 0xb2, u), lerp(0x74, 0x3a, u))
    } else {
        let u = (t - 0.5) / 0.5;
        Color32::from_rgb(lerp(0xe8, 0x6f, u), lerp(0xb2, 0xbf, u), lerp(0x3a, 0x73, u))
    }
}

enum Msg {
    Table(Table),
    Area(Area),
}

#[derive(Default)]
pub struct HfPanel {
    pub setup: Setup,
    pub map_freq: f64,
    pub map_hour: usize,
    pub show_area: bool,
    table: Option<Table>,
    pub area: Option<Area>,
    job: Option<Job<Msg>>,
    area_job: Option<Job<Msg>>,
    error: Option<String>,
    hover: Option<(usize, usize)>,
}

pub struct Ends<'a> {
    pub tx: (f64, f64),
    pub rx: (f64, f64),
    pub tx_ant: &'a dyn Fn(f64, f64) -> f64,
    pub rx_ant: &'a dyn Fn(f64, f64) -> f64,
    pub key: String,
}

impl HfPanel {
    pub fn new() -> Self {
        HfPanel { map_freq: 14.15, map_hour: 13, show_area: true, ..Default::default() }
    }

    pub fn sidebar(&mut self, ui: &mut Ui, site_transmits: &mut bool) {
        section(ui, "HF skywave", "ITU-R P.533, month median", |ui| {
            crate::path::role_row(ui, site_transmits);
            let s = &mut self.setup;
            row(ui, "month", |ui| {
                let names = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];
                egui::ComboBox::from_id_salt("hf-month").selected_text(names[s.month]).show_ui(
                    ui,
                    |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut s.month, i, *n);
                        }
                    },
                );
            });
            row_help(
                ui,
                "sunspots",
                "Smoothed sunspot number R12 for the month. About 100 to 150 near solar maximum, under 20 at minimum.",
                |ui| {
                    ui.add(egui::DragValue::new(&mut s.ssn).range(1..=300));
                },
            );
            row(ui, "power W", |ui| {
                ui.add(egui::DragValue::new(&mut s.watts).range(0.1..=100_000.0).speed(1.0));
            });
            row(ui, "bandwidth Hz", |ui| {
                ui.add(egui::DragValue::new(&mut s.bw).range(10.0..=20_000.0).speed(10.0));
            });
            row_help(
                ui,
                "needed SNR dB",
                "In the receiver bandwidth. Around 15 for SSB voice, 0 for CW in 500 Hz, -20 for FT8 in 2.5 kHz.",
                |ui| {
                    ui.add(egui::DragValue::new(&mut s.snr).range(-40.0..=60.0).speed(0.5));
                },
            );
            row(ui, "noise", |ui| {
                egui::ComboBox::from_id_salt("hf-noise").selected_text(s.noise.label()).show_ui(
                    ui,
                    |ui| {
                        for n in ManMade::ALL {
                            ui.selectable_value(&mut s.noise, n, n.label());
                        }
                    },
                );
            });
        });
        ui.add_space(8.0);
        section(ui, "sky map", "reliability from the transmitter", |ui| {
            row(ui, "map MHz", |ui| {
                ui.add(egui::DragValue::new(&mut self.map_freq).range(1.6..=30.0).speed(0.05));
            });
            row(ui, "UTC hour", |ui| {
                ui.add(egui::DragValue::new(&mut self.map_hour).range(1..=24));
            });
            if self.area_job.is_some() {
                progress(ui, "sky map", 0.0, None, "tracing every point on the globe");
            }
            hint(
                ui,
                "The chance of meeting the needed SNR from the transmitting end to anywhere, at that hour and frequency, into an isotropic antenna.",
            );
        });
    }

    pub fn poll(&mut self, ctx: &egui::Context, ends: &Ends) {
        for j in [&mut self.job, &mut self.area_job] {
            let msgs = j.as_mut().map(|j| j.poll()).unwrap_or_default();
            for m in msgs {
                match m {
                    Msg::Table(t) => {
                        self.table = Some(t);
                        *j = None;
                    }
                    Msg::Area(a) => {
                        self.area = Some(a);
                        *j = None;
                    }
                }
            }
        }
        let key = format!("{}|{}", ends.key, self.setup.key());
        let maps = match maps(self.setup.month) {
            Ready::Yes(m) => {
                self.error = None;
                m
            }
            Ready::Waiting => {
                ctx.request_repaint_after(web_time::Duration::from_millis(300));
                return;
            }
            Ready::Failed(e) => {
                self.error = Some(e);
                return;
            }
        };
        let pattern = |f: &dyn Fn(f64, f64) -> f64| Arc::new(Pattern::from_fn(f));
        if self.table.as_ref().is_none_or(|t| t.key != key) && self.job.is_none() {
            let (setup, tx, rx) = (self.setup.clone(), ends.tx, ends.rx);
            let (ta, ra) = (pattern(ends.tx_ant), pattern(ends.rx_ant));
            let (maps, k) = (maps.clone(), key.clone());
            self.job = Some(Job::spawn(ctx, "hf", move |h| {
                let noise = Coefficients::month(setup.month);
                let ctx = Context {
                    maps: &maps,
                    deciles: deciles(),
                    noise: &noise,
                    tx_ant: &ta,
                    rx_ant: &ra,
                };
                let cells = (0..24 * BANDS.len())
                    .into_par_iter()
                    .map(|i| {
                        run(&setup.input(tx, rx, i / BANDS.len(), BANDS[i % BANDS.len()].0), &ctx)
                    })
                    .collect();
                h.send(Msg::Table(Table { key: k, cells }));
            }));
        }
        let area_key = format!("{key}|{}|{}", self.map_hour, self.map_freq);
        if self.show_area
            && self.area.as_ref().is_none_or(|a| a.key != area_key)
            && self.area_job.is_none()
        {
            let (setup, tx) = (self.setup.clone(), ends.tx);
            let (ta, ra) = (pattern(ends.tx_ant), pattern(&|_, _| 0.0));
            let (hour, freq) = (self.map_hour.clamp(1, 24) - 1, self.map_freq);
            self.area_job = Some(Job::spawn(ctx, "hf area", move |h| {
                let noise = Coefficients::month(setup.month);
                let ctx = Context {
                    maps: &maps,
                    deciles: deciles(),
                    noise: &noise,
                    tx_ant: &ta,
                    rx_ant: &ra,
                };
                let bcr = (0..AREA_W * AREA_H)
                    .into_par_iter()
                    .map(|i| {
                        let lat = row_lat(i / AREA_W);
                        let lon = -180.0 + 360.0 * ((i % AREA_W) as f64 + 0.5) / AREA_W as f64;
                        run(&setup.input(tx, (lat, lon), hour, freq), &ctx).bcr as f32
                    })
                    .collect();
                h.send(Msg::Area(Area { key: area_key, hour: hour + 1, freq, bcr, tex: None }));
            }));
        }
    }

    pub fn draw_area(&mut self, ctx: &egui::Context, c: &crate::map::Canvas, opacity: f32) {
        if !self.show_area {
            return;
        }
        let Some(a) = &mut self.area else {
            return;
        };
        let tex = a.tex.get_or_insert_with(|| {
            let px: Vec<Color32> = a.bcr.iter().map(|v| reliability_colour(*v as f64)).collect();
            ctx.load_texture(
                "hf-area",
                ColorImage::new([AREA_W, AREA_H], px),
                egui::TextureOptions::LINEAR,
            )
        });
        c.p.image(
            tex.id(),
            Rect::from_min_max(c.at(MERC_LAT, -180.0), c.at(-MERC_LAT, 180.0)),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE.gamma_multiply(opacity),
        );
        let clip = c.p.clip_rect();
        c.p.text(
            Pos2::new(clip.left() + 8.0, clip.top() + 24.0),
            egui::Align2::LEFT_TOP,
            format!("HF reliability · {:.2} MHz · {:02} UTC", a.freq, a.hour),
            egui::FontId::monospace(10.5),
            Color32::WHITE,
        );
    }

    pub fn central(&mut self, ui: &mut Ui) {
        section(ui, "HF skywave", "reliability by hour and band, ITU-R P.533", |ui| {
            if let Some(e) = &self.error {
                status(ui, false, &format!("ionospheric maps: {e}"));
                return;
            }
            let Some(t) = &self.table else {
                note(
                    ui,
                    "Fetching the month's ionospheric maps from the ITU, about 11 MB…",
                    LEGEND,
                );
                return;
            };
            let w = ui.available_width();
            let label_w = 40.0;
            let rows = BANDS.len();
            let cell_h = 18.0;
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(w, cell_h * rows as f32 + 22.0), Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 2.0, WELL);
            let cell_w = (w - label_w - 8.0) / 24.0;
            let font = egui::FontId::monospace(10.0);
            self.hover = None;
            for (b, (_, name)) in BANDS.iter().enumerate() {
                let y = rect.top() + (rows - 1 - b) as f32 * cell_h;
                p.text(
                    Pos2::new(rect.left() + 6.0, y + cell_h / 2.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{name} m"),
                    font.clone(),
                    LEGEND,
                );
                for h in 0..24 {
                    let o = &t.cells[h * rows + b];
                    let r = Rect::from_min_size(
                        Pos2::new(rect.left() + label_w + h as f32 * cell_w, y + 1.0),
                        Vec2::new(cell_w - 1.0, cell_h - 2.0),
                    );
                    p.rect_filled(
                        r,
                        1.0,
                        reliability_colour(o.bcr).gamma_multiply(if o.bcr < 5.0 {
                            0.0
                        } else {
                            1.0
                        }),
                    );
                    if resp.hover_pos().is_some_and(|q| r.contains(q)) {
                        self.hover = Some((h, b));
                        p.rect_stroke(
                            r,
                            1.0,
                            Stroke::new(1.0, Color32::WHITE),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
            }
            for h in (0..24).step_by(3) {
                p.text(
                    Pos2::new(
                        rect.left() + label_w + (h as f32 + 0.5) * cell_w,
                        rect.bottom() - 10.0,
                    ),
                    egui::Align2::CENTER_CENTER,
                    format!("{:02}", h + 1),
                    font.clone(),
                    LEGEND,
                );
            }
            if let Some((h, b)) = self.hover {
                let o = &t.cells[h * rows + b];
                resp.on_hover_text(format!(
                    "{:02} UTC, {} MHz\nreliability {:.0}%, SNR {:.1} dB\nfield {:.1} dBµV/m, received {:.1} dBW\nMUF {:.1} MHz, operational {:.1} MHz, {:.0} km",
                    h + 1,
                    BANDS[b].0,
                    o.bcr,
                    o.snr,
                    o.ep,
                    o.pr,
                    o.muf50,
                    o.opmuf,
                    o.distance
                ));
            }
            let muf: Vec<(f64, f64)> = (0..24)
                .map(|h| (h as f64 + 1.0, t.cells[h * rows].muf50))
                .filter(|p| p.1 < 60.0)
                .collect();
            let op: Vec<(f64, f64)> = (0..24)
                .map(|h| (h as f64 + 1.0, t.cells[h * rows].opmuf))
                .filter(|p| p.1 < 60.0)
                .collect();
            let top = muf.iter().chain(&op).map(|p| p.1).fold(10.0, f64::max).ceil();
            Chart {
                x: (1.0, 24.0),
                y: (0.0, top),
                log_y: false,
                y_ticks: crate::charts::nice_ticks(0.0, top, 5),
                x_label: "UTC hour · MHz".into(),
                rules: Vec::new(),
                h_rules: Vec::new(),
                marks: Vec::new(),
                height: 150.0,
            }
            .show(
                ui,
                &[
                    Series { pts: muf, colour: GOLD, width: 2.0, label: "MUF".into() },
                    Series { pts: op, colour: GREEN, width: 1.6, label: "operational".into() },
                ],
            );
            let d = t.cells.first().map(|o| o.distance).unwrap_or(0.0);
            hint(
                ui,
                &format!(
                    "{d:.0} km. Each cell is the basic circuit reliability: the share of days in the month that the SNR meets what you need, from blue (rarely) through gold to green (nearly always). Antenna gains come from the patterns above, fixed gains otherwise."
                ),
            );
        });
    }
}
