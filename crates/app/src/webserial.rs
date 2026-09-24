use antenna_vna::{Point, shell};
use std::sync::mpsc::Receiver;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function vna_supported() {
    return typeof navigator !== "undefined" && "serial" in navigator;
}
export async function vna_open() {
    const port = await navigator.serial.requestPort({
        filters: [{ usbVendorId: 0x0483, usbProductId: 0x5740 }],
    });
    await port.open({ baudRate: 115200 });
    return { port, reader: port.readable.getReader(), writer: port.writable.getWriter(), pending: null };
}
export async function vna_write(h, text) {
    await h.writer.write(new TextEncoder().encode(text));
}
export async function vna_read(h, ms) {
    if (!h.pending) h.pending = h.reader.read();
    const timeout = new Promise((r) => setTimeout(() => r(null), ms));
    const got = await Promise.race([h.pending, timeout]);
    if (got === null) return new Uint8Array(0);
    h.pending = null;
    if (got.done) throw new Error("the port closed");
    return got.value;
}
export async function vna_close(h) {
    try { h.reader.releaseLock(); } catch (e) {}
    try { h.writer.releaseLock(); } catch (e) {}
    try { await h.port.close(); } catch (e) {}
}
export function vna_sleep(ms) {
    return new Promise((r) => setTimeout(r, ms));
}
export function vna_now() {
    return Date.now();
}
"#)]
extern "C" {
    pub fn vna_supported() -> bool;
    #[wasm_bindgen(catch)]
    async fn vna_open() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn vna_write(h: &JsValue, text: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn vna_read(h: &JsValue, ms: u32) -> Result<JsValue, JsValue>;
    async fn vna_close(h: &JsValue);
    async fn vna_sleep(ms: u32);
    fn vna_now() -> f64;
}

fn text(e: JsValue) -> String {
    e.as_string()
        .or_else(|| js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| "serial error".into())
}

pub struct Session {
    h: JsValue,
    pub max_points: usize,
    pub describe: String,
}

impl Session {
    pub async fn open() -> Result<Session, String> {
        let h = vna_open().await.map_err(text)?;
        let mut s = Session { h, max_points: 101, describe: String::new() };
        vna_write(&s.h, "\r").await.map_err(text)?;
        let until = vna_now() + 400.0;
        while vna_now() < until {
            let _ = vna_read(&s.h, 100).await;
        }
        let version = s.command("version", 3000.0).await?.join(" ");
        let info = s.command("info", 3000.0).await?;
        s.max_points = shell::max_points(&info);
        s.describe = format!(
            "{} · firmware {version} · {} points · Web Serial",
            info.first().cloned().unwrap_or_default(),
            s.max_points
        );
        Ok(s)
    }

    pub async fn command(&mut self, cmd: &str, timeout_ms: f64) -> Result<Vec<String>, String> {
        vna_write(&self.h, &format!("{cmd}\r")).await.map_err(text)?;
        let mut buf = Vec::new();
        let until = vna_now() + timeout_ms;
        loop {
            if let Some(r) = shell::reply(&buf, cmd) {
                return r.map_err(|e| e.to_string());
            }
            if vna_now() > until {
                return Err(format!("timed out waiting for `{cmd}`"));
            }
            let chunk = vna_read(&self.h, 200).await.map_err(text)?;
            buf.extend(js_sys::Uint8Array::new(&chunk).to_vec());
        }
    }

    pub async fn sweep(
        &mut self,
        start: f64,
        stop: f64,
        points: usize,
    ) -> Result<Vec<Point>, String> {
        let mut out = Vec::with_capacity(points);
        for (a, b, n, keep) in shell::chunks(start, stop, points, self.max_points) {
            let cmd = shell::scan_command(a, b, n, false);
            let lines = self.command(&cmd, 60_000.0).await?;
            out.extend(
                shell::parse_scan(&lines, false, n)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .take(keep),
            );
        }
        Ok(out)
    }

    pub async fn cal_status(&mut self) -> Option<String> {
        self.command("cal", 2000.0).await.ok().map(|l| l.join(" "))
    }

    pub async fn close(self) {
        vna_close(&self.h).await;
    }
}

pub async fn next_request(
    rx: &Receiver<(f64, f64, usize)>,
    alive: impl Fn() -> bool,
) -> Option<(f64, f64, usize)> {
    loop {
        if !alive() {
            return None;
        }
        match rx.try_recv() {
            Ok(r) => return Some(r),
            Err(std::sync::mpsc::TryRecvError::Empty) => vna_sleep(30).await,
            Err(_) => return None,
        }
    }
}
