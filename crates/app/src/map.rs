use egui::{
    Align2, Color32, ColorImage, Context, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};
use egui_bench::theme;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

pub const TILE_PX: f64 = 256.0;
const URL: &str = "https://tile.openstreetmap.org";
const CACHE_MAX: usize = 512;
const IN_FLIGHT: usize = 2;
const USER_AGENT: &str = concat!(
    "antenna-toolbox/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/v0l/antenna-toolbox)"
);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

enum Slot {
    Loading,
    Ready(TextureHandle),
    Failed,
}

type Done = (TileId, Result<ColorImage, String>);

pub struct Tiles {
    slots: HashMap<TileId, Slot>,
    order: Vec<TileId>,
    queue: Arc<Mutex<Vec<TileId>>>,
    wake: Vec<Sender<()>>,
    done: Receiver<Done>,
    error: Option<String>,
    failures: usize,
    ctx: Arc<Mutex<Option<Context>>>,
}

impl Default for Tiles {
    fn default() -> Self {
        Self::new()
    }
}

impl Tiles {
    pub fn new() -> Self {
        let queue: Arc<Mutex<Vec<TileId>>> = Arc::default();
        let ctx: Arc<Mutex<Option<Context>>> = Arc::default();
        let (done_tx, done) = channel();
        let dir = cache_dir();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(std::time::Duration::from_secs(15)))
            .build()
            .into();
        let wake = (0..IN_FLIGHT)
            .map(|_| {
                let (tx, rx) = channel::<()>();
                let (queue, done_tx, dir, agent, ctx) =
                    (queue.clone(), done_tx.clone(), dir.clone(), agent.clone(), ctx.clone());
                std::thread::Builder::new()
                    .name("tiles".into())
                    .spawn(move || {
                        while rx.recv().is_ok() {
                            loop {
                                let Some(id) = queue.lock().ok().and_then(|mut q| q.pop()) else {
                                    break;
                                };
                                let path = dir
                                    .as_ref()
                                    .map(|d| d.join(format!("{}/{}/{}.png", id.z, id.x, id.y)));
                                let out = load(&agent, id, path);
                                if done_tx.send((id, out)).is_err() {
                                    return;
                                }
                                if let Some(c) = ctx.lock().ok().and_then(|c| c.clone()) {
                                    c.request_repaint();
                                }
                            }
                        }
                    })
                    .expect("tile worker");
                tx
            })
            .collect();
        Self {
            slots: HashMap::new(),
            order: Vec::new(),
            queue,
            wake,
            done,
            error: None,
            failures: 0,
            ctx,
        }
    }

    pub fn poll(&mut self, ctx: &Context) {
        if let Ok(mut c) = self.ctx.lock()
            && c.is_none()
        {
            *c = Some(ctx.clone());
        }
        while let Ok((id, res)) = self.done.try_recv() {
            match res {
                Ok(img) => {
                    let tex = ctx.load_texture(
                        format!("tile-{}-{}-{}", id.z, id.x, id.y),
                        img,
                        TextureOptions::LINEAR,
                    );
                    self.slots.insert(id, Slot::Ready(tex));
                    self.error = None;
                }
                Err(e) => {
                    self.slots.insert(id, Slot::Failed);
                    self.failures += 1;
                    self.error = Some(e);
                }
            }
        }
    }

    pub fn get(&mut self, id: TileId) -> Option<&TextureHandle> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.slots.entry(id) {
            e.insert(Slot::Loading);
            self.order.push(id);
            if let Ok(mut q) = self.queue.lock() {
                q.push(id);
            }
            for w in &self.wake {
                let _ = w.send(());
            }
            self.evict();
        }
        match self.slots.get(&id) {
            Some(Slot::Ready(t)) => Some(t),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<(&str, usize)> {
        self.error.as_deref().map(|e| (e, self.failures))
    }

    fn evict(&mut self) {
        while self.order.len() > CACHE_MAX {
            let old = self.order.remove(0);
            if matches!(self.slots.get(&old), Some(Slot::Loading)) {
                self.order.push(old);
                break;
            }
            self.slots.remove(&old);
        }
    }
}

fn cache_dir() -> Option<PathBuf> {
    let dir = dirs::cache_dir()?.join("antenna-toolbox").join("tiles");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn load(agent: &ureq::Agent, id: TileId, path: Option<PathBuf>) -> Result<ColorImage, String> {
    if let Some(p) = path.as_deref()
        && let Ok(bytes) = std::fs::read(p)
        && let Ok(img) = decode(&bytes)
    {
        return Ok(img);
    }
    let url = format!("{URL}/{}/{}/{}.png", id.z, id.x, id.y);
    let bytes = agent
        .get(&url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .body_mut()
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))?;
    let img = decode(&bytes).map_err(|e| format!("{url}: {e}"))?;
    if let Some(p) = path {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, &bytes);
    }
    Ok(img)
}

fn decode(bytes: &[u8]) -> Result<ColorImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

pub fn project(lat: f64, lon: f64, z: u8) -> (f64, f64) {
    let n = f64::from(1u32 << z);
    let x = (lon + 180.0) / 360.0 * n;
    let s = lat.to_radians().sin().clamp(-0.9999, 0.9999);
    let y = (1.0 - ((1.0 + s) / (1.0 - s)).ln() / (2.0 * std::f64::consts::PI)) / 2.0 * n;
    (x, y)
}

pub fn unproject(x: f64, y: f64, z: u8) -> (f64, f64) {
    let n = f64::from(1u32 << z);
    let lon = x / n * 360.0 - 180.0;
    let t = std::f64::consts::PI * (1.0 - 2.0 * y / n);
    (t.sinh().atan().to_degrees(), lon)
}

pub fn level(zoom: f64) -> u8 {
    zoom.floor().clamp(2.0, 19.0) as u8
}

pub fn tile_scale(zoom: f64) -> f64 {
    TILE_PX * (zoom - f64::from(level(zoom))).exp2()
}

pub fn screen_to_ll(center: (f64, f64), zoom: f64, off: (f64, f64)) -> (f64, f64) {
    let (z, scale) = (level(zoom), tile_scale(zoom));
    let (cx, cy) = project(center.0, center.1, z);
    unproject(cx + off.0 / scale, cy + off.1 / scale, z)
}

pub fn ll_to_screen(center: (f64, f64), zoom: f64, ll: (f64, f64)) -> (f64, f64) {
    let (z, scale) = (level(zoom), tile_scale(zoom));
    let (cx, cy) = project(center.0, center.1, z);
    let (x, y) = project(ll.0, ll.1, z);
    ((x - cx) * scale, (y - cy) * scale)
}

pub fn anchored_zoom(center: (f64, f64), zoom: f64, new_zoom: f64, off: (f64, f64)) -> (f64, f64) {
    let anchor = screen_to_ll(center, zoom, off);
    let (z, scale) = (level(new_zoom), tile_scale(new_zoom));
    let (ax, ay) = project(anchor.0, anchor.1, z);
    let (lat, lon) = unproject(ax - off.0 / scale, ay - off.1 / scale, z);
    (lat.clamp(-85.0, 85.0), lon)
}

pub struct Camera {
    pub center: (f64, f64),
    pub zoom: f64,
}

pub struct Canvas {
    pub p: egui::Painter,
    mid: Pos2,
    center: (f64, f64),
    zoom: f64,
}

impl Canvas {
    pub fn at(&self, lat: f64, lon: f64) -> Pos2 {
        let (x, y) = ll_to_screen(self.center, self.zoom, (lat, lon));
        Pos2::new(self.mid.x + x as f32, self.mid.y + y as f32)
    }
}

pub struct Drawn {
    pub response: egui::Response,
    pub pointer: Option<(f64, f64)>,
}

#[derive(Default)]
pub struct MapView {
    pub camera: Option<Camera>,
    tiles: Tiles,
}

impl MapView {
    pub fn poll(&mut self, ctx: &Context) {
        self.tiles.poll(ctx);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        height: f32,
        fallback: (f64, f64),
        draw: impl FnOnce(&Canvas),
    ) -> Drawn {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 2.0, theme::WELL);
        let cam = self.camera.get_or_insert(Camera { center: fallback, zoom: 9.0 });
        let mid = rect.center();
        let offset = |pos: Pos2| (f64::from(pos.x - mid.x), f64::from(pos.y - mid.y));
        if resp.dragged() {
            let d = resp.drag_delta();
            cam.center = screen_to_ll(cam.center, cam.zoom, (f64::from(-d.x), f64::from(-d.y)));
        }
        if let (true, Some(pos)) = (resp.hovered(), resp.hover_pos()) {
            let d = ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta).y);
            if d != 0.0 {
                let next = (cam.zoom + f64::from(d) * 0.004).clamp(2.0, 18.0);
                cam.center = anchored_zoom(cam.center, cam.zoom, next, offset(pos));
                cam.zoom = next;
            }
        }
        let (center, zoom) = (cam.center, cam.zoom);
        let (z, scale) = (level(zoom), tile_scale(zoom));
        let (cx, cy) = project(center.0, center.1, z);
        let half_w = f64::from(rect.width()) / 2.0 / scale;
        let half_h = f64::from(rect.height()) / 2.0 / scale;
        let n = 1i64 << z;
        for ty in (cy - half_h).floor() as i64..=(cy + half_h).floor() as i64 {
            if ty < 0 || ty >= n {
                continue;
            }
            for tx in (cx - half_w).floor() as i64..=(cx + half_w).floor() as i64 {
                let id = TileId { z, x: tx.rem_euclid(n) as u32, y: ty as u32 };
                let Some(tex) = self.tiles.get(id) else {
                    continue;
                };
                let min = Pos2::new(
                    mid.x + ((tx as f64 - cx) * scale) as f32,
                    mid.y + ((ty as f64 - cy) * scale) as f32,
                );
                p.image(
                    tex.id(),
                    Rect::from_min_size(min, Vec2::splat(scale as f32)),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::from_gray(150),
                );
            }
        }
        let canvas = Canvas { p: p.clone(), mid, center, zoom };
        draw(&canvas);

        let font = theme::legend_font(10.0);
        p.text(
            Pos2::new(rect.left() + 8.0, rect.top() + 8.0),
            Align2::LEFT_TOP,
            format!("z{zoom:.1}    drag to pan · scroll to zoom · right-click to place"),
            font.clone(),
            theme::LEGEND,
        );
        let credit = "© OpenStreetMap contributors · ODbL";
        let g = p.layout_no_wrap(credit.into(), font.clone(), theme::VALUE);
        let plate = Rect::from_min_size(
            Pos2::new(rect.right() - g.size().x - 22.0, rect.bottom() - g.size().y - 16.0),
            g.size() + Vec2::new(14.0, 8.0),
        );
        p.rect_filled(plate, 3.0, Color32::from_black_alpha(205));
        p.galley(plate.min + Vec2::new(7.0, 4.0), g, theme::VALUE);
        if let Some((err, n)) = self.tiles.error() {
            let short: String = err.chars().take(110).collect();
            p.text(
                Pos2::new(rect.left() + 8.0, rect.bottom() - 10.0),
                Align2::LEFT_BOTTOM,
                format!("{n} tile(s) failed: {short}"),
                font,
                theme::FAULT,
            );
        }
        p.rect_stroke(rect, 2.0, Stroke::new(1.0, theme::ETCH), StrokeKind::Inside);
        let pointer = resp.hover_pos().map(|pos| screen_to_ll(center, zoom, offset(pos)));
        Drawn { response: resp, pointer }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_round_trips() {
        for (lat, lon) in [(53.35, -6.26), (0.0, 0.0), (-33.86, 151.21), (60.0, 179.0)] {
            let (x, y) = project(lat, lon, 11);
            let (la, lo) = unproject(x, y, 11);
            assert!((la - lat).abs() < 1e-9 && (lo - lon).abs() < 1e-9);
        }
    }

    #[test]
    fn zooming_leaves_the_point_under_the_pointer_alone() {
        let center = (53.35, -6.26);
        let off = (180.0, -95.0);
        for (from, to) in [(8.0, 9.3), (9.3, 8.0), (6.5, 14.0)] {
            let anchor = screen_to_ll(center, from, off);
            let moved = anchored_zoom(center, from, to, off);
            let (x, y) = ll_to_screen(moved, to, anchor);
            assert!((x - off.0).abs() < 1e-6 && (y - off.1).abs() < 1e-6);
        }
    }
}
