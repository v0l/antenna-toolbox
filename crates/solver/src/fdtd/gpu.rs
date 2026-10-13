use sims_field::Gpu;
use sims_field::gpu::Options;

fn options() -> Options {
    Options { label: "fdtd", ..Default::default() }
}

pub async fn adapter() -> Option<(wgpu::Adapter, String)> {
    let adapter = sims_field::gpu::adapter(&options()).await.ok()?;
    let info = adapter.get_info();
    Some((adapter, format!("{} ({:?})", info.name, info.backend)))
}

pub async fn open() -> Result<Gpu, String> {
    Gpu::open(&options()).await
}

#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
    pollster::block_on(f)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn available() -> bool {
    static HAVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HAVE.get_or_init(|| block_on(adapter()).is_some())
}
