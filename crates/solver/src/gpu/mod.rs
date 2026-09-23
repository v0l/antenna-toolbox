use crate::linalg::System;
use crate::surface::mesh::SurfaceTopology;
use crate::units::ETA;
use bytemuck::{Pod, Zeroable};
use num_complex::Complex64 as C64;
use std::sync::{Mutex, OnceLock};
use wgpu::util::DeviceExt;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub adapter: String,
    rwg_layout: wgpu::BindGroupLayout,
    rwg_fill: wgpu::ComputePipeline,
}

static GPU: OnceLock<Option<Mutex<Gpu>>> = OnceLock::new();

pub fn init() -> Option<String> {
    GPU.get_or_init(|| pollster::block_on(Gpu::new()).map(Mutex::new))
        .as_ref()
        .map(|g| g.lock().map(|g| g.adapter.clone()).unwrap_or_default())
}

pub fn ready() -> bool {
    GPU.get().is_some_and(|g| g.is_some())
}

pub fn disable() {
    let _ = GPU.set(None);
}

fn with_gpu<T>(f: impl FnOnce(&Gpu) -> Result<T, String>) -> Result<T, String> {
    let gpu = GPU.get().and_then(|g| g.as_ref()).ok_or("no GPU")?;
    let guard = gpu.lock().map_err(|_| "GPU lock poisoned")?;
    f(&guard)
}

pub fn fill_surface(topo: &SurfaceTopology, k: f64, feed: &[usize]) -> Result<System, String> {
    with_gpu(|g| g.fill_surface(topo, k, feed))
}

fn entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

const RO: wgpu::BufferBindingType = wgpu::BufferBindingType::Storage { read_only: true };
const RW: wgpu::BufferBindingType = wgpu::BufferBindingType::Storage { read_only: false };
const UNI: wgpu::BufferBindingType = wgpu::BufferBindingType::Uniform;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RwgUniform {
    n: u32,
    w: u32,
    col: u32,
    pad: u32,
    k: f32,
    eta: f32,
    pad2: f32,
    pad3: f32,
}

impl Gpu {
    async fn new() -> Option<Gpu> {
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
        let have = adapter.limits();
        let want: u64 = 512 * 1024 * 1024;
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: want.min(have.max_storage_buffer_binding_size),
            max_buffer_size: want.min(have.max_buffer_size),
            ..wgpu::Limits::downlevel_defaults().using_resolution(have.clone())
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("antenna-solver"),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .ok()?;

        let pipe = |layout: &wgpu::BindGroupLayout, module: &wgpu::ShaderModule, name: &str| {
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(name),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pl),
                module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let module = |src: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };

        let rwg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rwg"),
            entries: &[
                entry(0, RO),
                entry(1, RO),
                entry(2, RO),
                entry(3, RW),
                entry(4, UNI),
            ],
        });
        let rwg_mod = module(include_str!("rwg.wgsl"));
        let rwg_fill = pipe(&rwg_layout, &rwg_mod, "fill_rwg");

        Some(Gpu {
            adapter: format!("{} ({:?})", info.name, info.backend),
            device,
            queue,
            rwg_layout,
            rwg_fill,
        })
    }

    fn storage(&self, contents: &[u8], read_only: bool) -> wgpu::Buffer {
        let usage = if read_only {
            wgpu::BufferUsages::STORAGE
        } else {
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST
        };
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents,
            usage,
        })
    }

    fn matrix(&self, n: usize) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("A"),
            size: (n * (n + 1) * 8) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn uniform(&self, contents: &[u8]) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents,
            usage: wgpu::BufferUsages::UNIFORM,
        })
    }

    fn dispatch_fill(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind: &wgpu::BindGroup,
        n: usize,
    ) -> wgpu::CommandBuffer {
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind, &[]);
            let groups = (n as u32).div_ceil(8);
            pass.dispatch_workgroups(groups, groups, 1);
        }
        enc.finish()
    }

    fn read_matrix(&self, buf: &wgpu::Buffer, n: usize) -> Result<Vec<C64>, String> {
        let size = (n * (n + 1) * 8) as u64;
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(buf, 0, &read, 0, size);
        self.queue.submit([enc.finish()]);
        let slice = read.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let data = slice.get_mapped_range();
        let floats: &[f32] = bytemuck::cast_slice(&data);
        let out: Vec<C64> =
            floats.chunks_exact(2).map(|c| C64::new(c[0] as f64, c[1] as f64)).collect();
        drop(data);
        read.unmap();
        if out.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
            return Err("GPU produced a non-finite entry".into());
        }
        Ok(out)
    }

    fn fill_surface(
        &self,
        topo: &SurfaceTopology,
        k: f64,
        feed: &[usize],
    ) -> Result<System, String> {
        let n = topo.edges.len();
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let v = &topo.mesh.vertices;
        let tri_data: Vec<f32> = topo
            .triangles
            .iter()
            .flat_map(|t| {
                let (a, b, c) = (v[t.v[0]], v[t.v[1]], v[t.v[2]]);
                [
                    a[0], a[1], a[2], t.area, b[0], b[1], b[2], 0.0, c[0], c[1], c[2], 0.0,
                    t.centre[0], t.centre[1], t.centre[2], 0.0, t.normal[0], t.normal[1],
                    t.normal[2], 0.0,
                ]
                .map(|x| x as f32)
            })
            .collect();
        let edge_data: Vec<u32> = topo
            .edges
            .iter()
            .flat_map(|e| {
                [
                    (e.length as f32).to_bits(),
                    e.plus as u32,
                    e.minus as u32,
                    e.free_plus as u32,
                    e.free_minus as u32,
                    0,
                    0,
                    0,
                ]
            })
            .collect();
        let vert_data: Vec<f32> =
            v.iter().flat_map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, 0.0]).collect();
        let tri_buf = self.storage(bytemuck::cast_slice(&tri_data), true);
        let edge_buf = self.storage(bytemuck::cast_slice(&edge_data), true);
        let vert_buf = self.storage(bytemuck::cast_slice(&vert_data), true);
        let a_buf = self.matrix(n);
        let u = RwgUniform {
            n: n as u32,
            w: (n + 1) as u32,
            col: 0,
            pad: 0,
            k: k as f32,
            eta: ETA as f32,
            pad2: 0.0,
            pad3: 0.0,
        };
        let u_buf = self.uniform(bytemuck::bytes_of(&u));
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.rwg_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: tri_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: edge_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: vert_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: a_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: u_buf.as_entire_binding() },
            ],
        });
        self.queue.submit([self.dispatch_fill(&self.rwg_fill, &bind, n)]);
        let a_host = self.read_matrix(&a_buf, n)?;
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(e.to_string());
        }
        let mut sys = System { a: a_host, n, w: n + 1 };
        for &f in feed {
            *sys.rhs_mut(f) = C64::new(topo.edges[f].length, 0.0);
        }
        Ok(sys)
    }
}
