use crate::linalg::System;
use crate::surface::mesh::SurfaceTopology;
use crate::units::ETA;
use bytemuck::{Pod, Zeroable};
use num_complex::Complex64 as C64;
use std::sync::{Mutex, OnceLock};

pub struct Gpu {
    gpu: fieldgrid::Gpu,
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
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
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
        let options = fieldgrid::gpu::Options { label: "antenna-solver", ..Default::default() };
        let gpu = fieldgrid::Gpu::open(&options).await.ok()?;
        let device = &gpu.device;
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
            entries: &[entry(0, RO), entry(1, RO), entry(2, RO), entry(3, RW), entry(4, UNI)],
        });
        let rwg_mod = module(include_str!("rwg.wgsl"));
        let rwg_fill = pipe(&rwg_layout, &rwg_mod, "fill_rwg");

        Some(Gpu { adapter: gpu.name.clone(), gpu, rwg_layout, rwg_fill })
    }

    fn dispatch_fill(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bind: &wgpu::BindGroup,
        n: usize,
    ) -> wgpu::CommandBuffer {
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
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
        let floats: Vec<f32> = self.gpu.read(buf, 0..2 * n * (n + 1))?;
        let out: Vec<C64> =
            floats.chunks_exact(2).map(|c| C64::new(c[0] as f64, c[1] as f64)).collect();
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
        let scope = self.gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let v = &topo.mesh.vertices;
        let tri_data: Vec<f32> = topo
            .triangles
            .iter()
            .flat_map(|t| {
                let (a, b, c) = (v[t.v[0]], v[t.v[1]], v[t.v[2]]);
                [
                    a[0],
                    a[1],
                    a[2],
                    t.area,
                    b[0],
                    b[1],
                    b[2],
                    0.0,
                    c[0],
                    c[1],
                    c[2],
                    0.0,
                    t.centre[0],
                    t.centre[1],
                    t.centre[2],
                    0.0,
                    t.normal[0],
                    t.normal[1],
                    t.normal[2],
                    0.0,
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
        let tri_buf = self.gpu.storage("triangles", &tri_data);
        let edge_buf = self.gpu.storage("edges", &edge_data);
        let vert_buf = self.gpu.storage("vertices", &vert_data);
        let a_buf = self.gpu.zeroed("A", (n * (n + 1) * 8) as u64);
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
        let u_buf = self.gpu.uniform("rwg", &u);
        let bind = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        self.gpu.queue.submit([self.dispatch_fill(&self.rwg_fill, &bind, n)]);
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
