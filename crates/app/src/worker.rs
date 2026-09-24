use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

pub struct Job<T> {
    rx: Receiver<T>,
    cancel: Arc<AtomicBool>,
    ctx: Option<egui::Context>,
}

#[derive(Clone)]
pub struct Handle<T> {
    tx: Sender<T>,
    cancel: Arc<AtomicBool>,
    ctx: egui::Context,
}

impl<T> Handle<T> {
    pub fn send(&self, msg: T) -> bool {
        let ok = !self.cancelled() && self.tx.send(msg).is_ok();
        if !cfg!(target_arch = "wasm32") {
            self.ctx.request_repaint();
        }
        ok
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(
        ctx: &egui::Context,
        name: &str,
        f: impl FnOnce(Handle<T>) + Send + 'static,
    ) -> Self {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let handle = Handle { tx, cancel: cancel.clone(), ctx: ctx.clone() };
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || f(handle))
            .expect("spawn worker");
        #[cfg(target_arch = "wasm32")]
        {
            let _ = name;
            rayon::spawn(move || f(handle));
        }
        Job { rx, cancel, ctx: Some(ctx.clone()) }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spawn_async<F: std::future::Future<Output = ()> + 'static>(
        ctx: &egui::Context,
        f: impl FnOnce(Handle<T>) -> F,
    ) -> Self {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let handle = Handle { tx, cancel: cancel.clone(), ctx: ctx.clone() };
        wasm_bindgen_futures::spawn_local(f(handle));
        Job { rx, cancel, ctx: Some(ctx.clone()) }
    }

    #[cfg(test)]
    pub fn from_receiver(rx: Receiver<T>) -> Self {
        Job { rx, cancel: Arc::new(AtomicBool::new(false)), ctx: None }
    }

    pub fn poll(&mut self) -> Vec<T> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(m) => out.push(m),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if cfg!(target_arch = "wasm32")
                        && let Some(c) = &self.ctx
                    {
                        c.request_repaint_after(web_time::Duration::from_millis(50));
                    }
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        out
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
