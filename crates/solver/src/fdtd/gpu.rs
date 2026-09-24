use super::engine::Sim;
use super::ntff;
use bytemuck::{Pod, Zeroable};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    n: [u32; 4],
    pml: u32,
    port_count: u32,
    port_axis: u32,
    patches: u32,
    psi: [u32; 20],
    dt: f32,
    k_mu: f32,
    t0: f32,
    tau: f32,
    w0: f32,
    wf: f32,
    series_cap: u32,
    pad: u32,
    cur: [u32; 4],
    cur_d: [f32; 4],
}

struct Shot<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

struct Oneshot<T>(Arc<Mutex<Shot<T>>>);

impl<T> Future for Oneshot<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let Ok(mut s) = self.0.lock() else {
            return Poll::Pending;
        };
        match s.value.take() {
            Some(v) => Poll::Ready(v),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub struct GpuFdtd {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipes: Vec<(wgpu::ComputePipeline, Vec<wgpu::BindGroup>, [u32; 3])>,
    energy: (wgpu::ComputePipeline, Vec<wgpu::BindGroup>),
    series: wgpu::Buffer,
    partial: wgpu::Buffer,
    acc: wgpu::Buffer,
    patches: usize,
    pub cap: usize,
    pub adapter: String,
}

pub async fn adapter() -> Option<(wgpu::Adapter, String)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .ok()?;
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu {
        return None;
    }
    Some((adapter, format!("{} ({:?})", info.name, info.backend)))
}

impl GpuFdtd {
    pub async fn new(
        sim: &Sim,
        far: Option<&ntff::Box>,
        f0: f64,
        fc: f64,
        cap: usize,
    ) -> Result<GpuFdtd, String> {
        let (adapter, name) = adapter().await.ok_or("no GPU adapter")?;
        let n = sim.dims();
        let nn = n[0] * n[1] * n[2];
        let have = adapter.limits();
        let need = (6 * nn * 4) as u64;
        if need > have.max_storage_buffer_binding_size || need > have.max_buffer_size {
            return Err(format!("{} cells is more than this GPU can hold", sim.grid.cells()));
        }
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: have.max_storage_buffer_binding_size,
            max_buffer_size: have.max_buffer_size,
            max_storage_buffers_per_shader_stage: have.max_storage_buffers_per_shader_stage.min(8),
            ..wgpu::Limits::downlevel_defaults().using_resolution(have.clone())
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("fdtd"),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let storage = |data: &[u8], label: &str| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: data,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            })
        };
        let mut fields = Vec::with_capacity(6 * nn);
        for c in 0..3 {
            fields.extend_from_slice(&sim.e[c]);
        }
        for c in 0..3 {
            fields.extend_from_slice(&sim.h[c]);
        }
        let mut coef = Vec::with_capacity(6 * nn);
        for c in 0..3 {
            coef.extend_from_slice(&sim.ca[c]);
        }
        for c in 0..3 {
            coef.extend_from_slice(&sim.cb[c]);
        }
        let mut psi = Vec::new();
        let mut offsets = [0u32; 20];
        for (e, set) in [&sim.psi_e, &sim.psi_h].iter().enumerate() {
            for c in 0..3 {
                for a in 0..3 {
                    offsets[e * 9 + c * 3 + a] = psi.len() as u32;
                    psi.extend_from_slice(&set[c][a]);
                }
            }
        }
        if psi.is_empty() {
            psi.push(0.0);
        }
        let mut axes = Vec::new();
        for a in &sim.ax {
            for i in 0..a.n {
                axes.extend_from_slice(&[
                    a.inv_d[i],
                    a.inv_dd[i],
                    a.raw_inv_d[i],
                    a.raw_inv_dd[i],
                    a.be[i],
                    a.ce[i],
                    a.bh[i],
                    a.ch[i],
                    a.slot_e[i] as f32,
                    a.slot_h[i] as f32,
                ]);
            }
        }
        let port = sim.port.ok_or("no port")?;
        let mut port_data = Vec::new();
        for &(id, src, dl, _) in &sim.port_coef {
            port_data.extend_from_slice(&[id as f32, src, dl]);
        }
        let (ids, dd_v, dd_u, u, v) = sim.current_probe().ok_or("no port")?;
        let (t0, tau) = super::engine::pulse_shape(fc);
        let patch_idx = far.map(|b| b.gpu_indices(sim)).unwrap_or_else(|| vec![0; 13]);
        let patches = far.map(|b| b.patch_count()).unwrap_or(0);
        if nn >= (1 << 24) || ids.iter().any(|i| *i >= (1 << 24)) {
            return Err("grid too large for the GPU port indexing".into());
        }
        let params = Params {
            n: [n[0] as u32, n[1] as u32, n[2] as u32, nn as u32],
            pml: sim.pml as u32,
            port_count: sim.port_coef.len() as u32,
            port_axis: port.axis as u32,
            patches: patches as u32,
            psi: offsets,
            dt: sim.dt as f32,
            k_mu: (sim.dt / super::MU0) as f32,
            t0: t0 as f32,
            tau: tau as f32,
            w0: (2.0 * std::f64::consts::PI * f0) as f32,
            wf: far.map(|b| 2.0 * std::f64::consts::PI * b.freq).unwrap_or(0.0) as f32,
            series_cap: cap as u32,
            pad: 0,
            cur: [ids[0] as u32, ids[1] as u32, ids[2] as u32, (v * 4 + u) as u32],
            cur_d: [dd_v as f32, dd_u as f32, 0.0, 0.0],
        };
        let b_fields = storage(bytemuck::cast_slice(&fields), "fields");
        let b_coef = storage(bytemuck::cast_slice(&coef), "coef");
        let b_psi = storage(bytemuck::cast_slice(&psi), "psi");
        let b_axes = storage(bytemuck::cast_slice(&axes), "axes");
        let b_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let b_state = storage(bytemuck::cast_slice(&[0u32; 4]), "state");
        let series = storage(bytemuck::cast_slice(&vec![0f32; 2 * cap]), "series");
        let b_port = storage(bytemuck::cast_slice(&port_data), "port");
        let b_patch = storage(bytemuck::cast_slice(&patch_idx), "patch");
        let acc = storage(bytemuck::cast_slice(&vec![0f32; 12 * patches.max(1)]), "acc");
        let partial = storage(bytemuck::cast_slice(&vec![0f32; 1024]), "partial");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fdtd"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fdtd.wgsl").into()),
        });
        let group0: [(u32, &wgpu::Buffer); 8] = [
            (0, &b_fields),
            (1, &b_coef),
            (2, &b_psi),
            (3, &b_axes),
            (4, &b_params),
            (5, &b_state),
            (6, &series),
            (7, &b_port),
        ];
        let group1: [(u32, &wgpu::Buffer); 3] = [(0, &b_patch), (1, &acc), (2, &partial)];
        let make = |entry: &str, uses0: &[u32], uses1: &[u32]| {
            let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            });
            let mut groups = Vec::new();
            for (g, (uses, set)) in [(uses0, &group0[..]), (uses1, &group1[..])].iter().enumerate()
            {
                if uses.is_empty() {
                    continue;
                }
                let entries: Vec<wgpu::BindGroupEntry> = set
                    .iter()
                    .filter(|(b, _)| uses.contains(b))
                    .map(|(b, buf)| wgpu::BindGroupEntry {
                        binding: *b,
                        resource: buf.as_entire_binding(),
                    })
                    .collect();
                groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(entry),
                    layout: &pipe.get_bind_group_layout(g as u32),
                    entries: &entries,
                }));
            }
            (pipe, groups)
        };
        let grid = [(n[2] as u32).div_ceil(64), (n[1] as u32).div_ceil(4), n[0] as u32];
        let (h, hg) = make("update_h", &[0, 2, 3, 4], &[]);
        let (e, eg) = make("update_e", &[0, 1, 2, 3, 4], &[]);
        let (s, sg) = make("source", &[0, 4, 5, 7], &[]);
        let (p, pg) = make("probe", &[0, 4, 5, 6, 7], &[]);
        let mut pipes = vec![
            (h, hg, grid),
            (e, eg, grid),
            (s, sg, [(sim.port_coef.len() as u32).div_ceil(64), 1, 1]),
        ];
        if patches > 0 {
            let (f, fg) = make("ntff", &[0, 4, 5], &[0, 1]);
            pipes.push((f, fg, [(patches as u32).div_ceil(64), 1, 1]));
        }
        pipes.push((p, pg, [1, 1, 1]));
        let energy = make("energy", &[0, 4], &[2]);
        Ok(GpuFdtd {
            device,
            queue,
            pipes,
            energy,
            series,
            partial,
            acc,
            patches,
            cap,
            adapter: name,
        })
    }

    pub fn advance(&self, steps: usize) {
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            for _ in 0..steps {
                for (pipe, groups, dispatch) in &self.pipes {
                    pass.set_pipeline(pipe);
                    for (g, bg) in groups.iter().enumerate() {
                        pass.set_bind_group(g as u32, bg, &[]);
                    }
                    pass.dispatch_workgroups(dispatch[0], dispatch[1], dispatch[2]);
                }
            }
        }
        self.queue.submit([enc.finish()]);
    }

    async fn read(&self, buf: &wgpu::Buffer, offset: u64, size: u64) -> Result<Vec<f32>, String> {
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(buf, offset, &read, 0, size);
        self.queue.submit([enc.finish()]);
        let slot = Arc::new(Mutex::new(Shot { value: None, waker: None }));
        let tx = slot.clone();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            if let Ok(mut s) = tx.lock() {
                s.value = Some(r);
                if let Some(w) = s.waker.take() {
                    w.wake();
                }
            }
        });
        #[cfg(not(target_arch = "wasm32"))]
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        Oneshot(slot).await.map_err(|e| e.to_string())?;
        let data = read.slice(..).get_mapped_range();
        let out = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
        drop(data);
        read.unmap();
        Ok(out)
    }

    pub async fn series(&self, from: usize, to: usize) -> Result<Vec<f32>, String> {
        let to = to.min(self.cap);
        if to <= from {
            return Ok(Vec::new());
        }
        self.read(&self.series, (from * 8) as u64, ((to - from) * 8) as u64).await
    }

    pub async fn energy(&self) -> Result<f64, String> {
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.energy.0);
            for (g, bg) in self.energy.1.iter().enumerate() {
                pass.set_bind_group(g as u32, bg, &[]);
            }
            pass.dispatch_workgroups(1024, 1, 1);
        }
        self.queue.submit([enc.finish()]);
        let v = self.read(&self.partial, 0, 4096).await?;
        Ok(v.iter().map(|x| *x as f64).sum())
    }

    pub async fn far_field(&self) -> Result<Vec<f32>, String> {
        if self.patches == 0 {
            return Ok(Vec::new());
        }
        self.read(&self.acc, 0, (self.patches * 12 * 4) as u64).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<T>(f: impl Future<Output = T>) -> T {
    pollster::block_on(f)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn available() -> bool {
    static HAVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HAVE.get_or_init(|| block_on(adapter()).is_some())
}
