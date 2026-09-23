use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

pub struct Job<T> {
    rx: Receiver<T>,
    cancel: Arc<AtomicBool>,
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
        self.ctx.request_repaint();
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
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || f(handle))
            .expect("spawn worker");
        Job { rx, cancel }
    }

    #[cfg(test)]
    pub fn from_receiver(rx: Receiver<T>) -> Self {
        Job { rx, cancel: Arc::new(AtomicBool::new(false)) }
    }

    pub fn poll(&mut self) -> Vec<T> {
        self.rx.try_iter().collect()
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
