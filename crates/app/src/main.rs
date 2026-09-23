#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 920.0])
            .with_title("antenna toolbox"),
        ..Default::default()
    };
    eframe::run_native("antenna-toolbox", options, Box::new(|cc| Ok(antenna_toolbox::start(cc))))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
