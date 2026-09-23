mod charts;
mod design;
mod drawing;
mod map;
mod path;
mod rich;
mod state;
mod traces;
mod view3d;
mod vna;
mod worker;

use design::DesignTab;
use egui_bench::prelude::*;
use path::PathTab;
use std::sync::{Arc, Mutex};
use vna::VnaTab;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Design,
    Vna,
    Path,
}

struct App {
    saved: String,
    page_offset: f32,
    tab: Tab,
    design: DesignTab,
    vna: VnaTab,
    path: PathTab,
    gpu: Arc<Mutex<Option<String>>>,
}

impl App {
    fn new() -> Self {
        let gpu: Arc<Mutex<Option<String>>> = Arc::default();
        let slot = gpu.clone();
        if !cfg!(test) {
            std::thread::spawn(move || {
                let name = antenna_solver::gpu::init().unwrap_or_else(|| "none, CPU only".into());
                if let Ok(mut g) = slot.lock() {
                    *g = Some(name);
                }
            });
        }
        let mut app = Self {
            saved: String::new(),
            page_offset: 0.0,
            tab: Tab::Design,
            design: DesignTab::default(),
            vna: VnaTab::default(),
            path: PathTab::default(),
            gpu,
        };
        if !cfg!(test) {
            state::load(&mut app);
        }
        app.saved = format!("{:?}", state::snapshot(&app));
        app
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.design.poll(&ctx);
        self.vna.poll();
        self.path.poll(&ctx);
        let now = format!("{:?}", state::snapshot(self));
        if !cfg!(test) && now != self.saved && !ui.input(|i| i.pointer.any_down()) {
            state::save(self);
            self.saved = now;
        }

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                modal_title(ui, "antenna toolbox");
                ui.add_space(16.0);
                let gpu = self
                    .gpu
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .unwrap_or_else(|| "starting".into());
                Line::new().legend("gpu").gap(6.0).value(gpu).size(11.0).show(ui);
            });
            tabs(
                ui,
                &mut self.tab,
                &[(Tab::Design, "design"), (Tab::Vna, "vna"), (Tab::Path, "path")],
            );
        });

        egui::Panel::left("side").exact_size(330.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);
                match self.tab {
                    Tab::Design => self.design.sidebar(ui),
                    Tab::Vna => self.vna.sidebar(ui),
                    Tab::Path => self.path.sidebar(ui),
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let out = egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                match self.tab {
                    Tab::Design => self.design.central(ui),
                    Tab::Vna => self.vna.central(ui),
                    Tab::Path => self.path.central(ui),
                }
                ui.add_space(20.0);
            });
            self.page_offset = out.state.offset.y;
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 920.0])
            .with_title("antenna toolbox"),
        ..Default::default()
    };
    eframe::run_native(
        "antenna-toolbox",
        options,
        Box::new(|cc| {
            egui_bench::install(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    fn settle(h: &mut Harness<'_, App>, ms: u64) {
        for _ in 0..ms / 50 {
            h.step();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn wheel(h: &mut Harness<'_, App>, at: egui::Pos2, dy: f32) {
        h.hover_at(at);
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, dy),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(20);
    }

    #[test]
    fn scrolling_over_the_map_zooms_it_and_leaves_the_page_alone() {
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 490.0)).build_eframe(|cc| {
            egui_bench::install(&cc.egui_ctx);
            App::new()
        });
        h.state_mut().tab = Tab::Path;
        h.run_steps(3);
        let zoom = h.state().path.map_zoom();
        wheel(&mut h, egui::pos2(800.0, 200.0), -120.0);
        assert!(h.state().path.map_zoom() < zoom, "map did not zoom out");
        assert_eq!(h.state().page_offset, 0.0, "page scrolled under the map");
        wheel(&mut h, egui::pos2(800.0, 474.0), -120.0);
        assert!(
            h.state().page_offset > 0.0,
            "page does not scroll at all, so the test proves nothing"
        );
    }

    #[test]
    #[ignore = "renders the app to target/shots for a visual check"]
    fn render_every_tab() {
        let mut h =
            Harness::builder().with_size(egui::vec2(1400.0, 1000.0)).wgpu().build_eframe(|cc| {
                egui_bench::install(&cc.egui_ctx);
                App::new()
            });
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shots");
        std::fs::create_dir_all(&dir).unwrap();
        let designs: Vec<String> = std::env::var("SHOT_DESIGNS")
            .map(|s| s.split(',').map(str::to_string).collect())
            .unwrap_or_else(|_| vec!["moxon".into()]);
        for id in &designs {
            h.state_mut().design.design = antenna_designs::by_id(id);
            h.state_mut().design.freq =
                std::env::var("SHOT_FREQ").ok().and_then(|f| f.parse().ok()).unwrap_or(162.0);
            settle(&mut h, 2500);
            h.render().unwrap().save(dir.join(format!("design-{id}.png"))).unwrap();
        }
        for (tab, name) in [(Tab::Vna, "vna"), (Tab::Path, "path")] {
            h.state_mut().tab = tab;
            settle(&mut h, 4000);
            h.render().unwrap().save(dir.join(format!("{name}.png"))).unwrap();
        }
    }

    #[test]
    #[ignore = "renders a synthetic sweep to target/shots"]
    fn render_vna_display() {
        let mut h =
            Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).wgpu().build_eframe(|cc| {
                egui_bench::install(&cc.egui_ctx);
                App::new()
            });
        let pts = (0..401)
            .map(|i| {
                let f = 800e6 + i as f64 * 0.25e6;
                let w = f / 880e6;
                let z = antenna_vna::C64::new(62.0, 900.0 * (w - 1.0 / w));
                let s11 = (z - 50.0) / (z + 50.0);
                antenna_vna::Point { freq: f, s11, s21: None }
            })
            .collect();
        h.state_mut().vna.target = 868.0;
        h.state_mut().vna.inject(pts);
        h.state_mut().tab = Tab::Vna;
        h.run_steps(5);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shots");
        std::fs::create_dir_all(&dir).unwrap();
        h.render().unwrap().save(dir.join("vna-display.png")).unwrap();
    }

    #[test]
    #[ignore = "needs a VNA on USB and renders to target/shots"]
    fn render_live_vna() {
        let mut h =
            Harness::builder().with_size(egui::vec2(1400.0, 1000.0)).wgpu().build_eframe(|cc| {
                egui_bench::install(&cc.egui_ctx);
                App::new()
            });
        let freq = std::env::var("SHOT_FREQ").ok().and_then(|f| f.parse().ok()).unwrap_or(868.0);
        h.state_mut().vna.target = freq;
        h.state_mut().tab = Tab::Vna;
        settle(&mut h, 2500);
        let ctx = h.ctx.clone();
        h.state_mut().vna.live_sweep(&ctx);
        settle(&mut h, 6000);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shots");
        std::fs::create_dir_all(&dir).unwrap();
        h.render().unwrap().save(dir.join("vna-live.png")).unwrap();
    }
}
