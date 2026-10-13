pub mod engine;
pub mod gpu;
pub mod mesh;
pub mod ntff;

use num_complex::Complex64 as C64;

pub const C0: f64 = 299_792_458.0;
pub const EPS0: f64 = 8.854_187_812_8e-12;
pub const MU0: f64 = 1.256_637_062_12e-6;
pub const ETA0: f64 = 376.730_313_668;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

impl Aabb {
    pub fn new(a: [f64; 3], b: [f64; 3]) -> Aabb {
        Aabb { lo: [0, 1, 2].map(|i| a[i].min(b[i])), hi: [0, 1, 2].map(|i| a[i].max(b[i])) }
    }

    pub fn contains_segment(&self, a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|t| a[t].min(b[t]) >= self.lo[t] - tol && a[t].max(b[t]) <= self.hi[t] + tol)
    }

    pub fn contains(&self, p: [f64; 3]) -> bool {
        (0..3).all(|t| p[t] >= self.lo[t] && p[t] <= self.hi[t])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dielectric {
    pub bounds: Aabb,
    pub eps_r: f64,
    pub tan_d: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub r: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub metal: Vec<Aabb>,
    pub dielectrics: Vec<Dielectric>,
    pub port: Option<Port>,
    pub f0: f64,
    pub span: f64,
    pub fine: f64,
    pub ground_plane_z: Option<f64>,
}

impl Model {
    pub fn bounds(&self) -> Option<Aabb> {
        let mut it = self
            .metal
            .iter()
            .copied()
            .chain(self.dielectrics.iter().map(|d| d.bounds))
            .chain(self.port.iter().map(|p| Aabb::new(p.a, p.b)));
        let first = it.next()?;
        Some(it.fold(first, |acc, b| Aabb {
            lo: [0, 1, 2].map(|i| acc.lo[i].min(b.lo[i])),
            hi: [0, 1, 2].map(|i| acc.hi[i].max(b.hi[i])),
        }))
    }
}

#[derive(Clone, Debug)]
pub struct Sweep {
    pub freqs: Vec<f64>,
    pub z: Vec<C64>,
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub cells: usize,
    pub steps: usize,
    pub dt: f64,
    pub decay_db: f64,
    pub gpu: bool,
}

pub struct Outcome {
    pub sweep: Sweep,
    pub z0: C64,
    pub p_in: f64,
    pub far: Option<ntff::Box>,
    pub stats: Stats,
    pub grid: mesh::Grid,
}

pub struct Options {
    pub points: usize,
    pub max_steps: usize,
    pub decay_db: f64,
    pub pml: usize,
    pub far_field: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { points: 121, max_steps: 200_000, decay_db: 50.0, pml: 8, far_field: true }
    }
}

struct Setup {
    sim: engine::Sim,
    patches: Option<fieldgrid::fdtd::Patches>,
    freqs: Vec<f64>,
    f0: f64,
    fc: f64,
    grid: mesh::Grid,
}

fn setup(model: &Model, opts: &Options) -> Result<Setup, String> {
    let port = model.port.ok_or("the model has no port")?;
    let grid = mesh::mesh(model, opts.pml).ok_or("nothing to mesh")?;
    let sim = engine::build(grid.clone(), model, port);
    let f0 = model.f0;
    let fc = f0 * 0.5;
    let freqs: Vec<f64> = (0..opts.points)
        .map(|i| {
            f0 * (1.0 - model.span + 2.0 * model.span * i as f64 / (opts.points - 1).max(1) as f64)
        })
        .collect();
    let patches = opts.far_field.then(|| fieldgrid::fdtd::patches(&sim, 3));
    Ok(Setup { sim, patches, freqs, f0, fc, grid })
}

fn extras(s: &Setup, opts: &Options) -> fieldgrid::fdtd::Extras {
    fieldgrid::fdtd::Extras {
        max_steps: opts.max_steps,
        decay_db: opts.decay_db,
        freqs: if s.patches.is_some() { vec![s.f0] } else { Vec::new() },
        ntff: s.patches.as_ref().map(|p| p.gpu.clone()),
        check_every: 500,
        ..Default::default()
    }
}

fn finish(s: Setup, rec: fieldgrid::fdtd::Record, gpu: bool) -> Outcome {
    let dt = s.sim.dt;
    let series = &rec.series;
    let all: Vec<f64> = s.freqs.iter().copied().chain(std::iter::once(s.f0)).collect();
    let spectra: Vec<(C64, C64)> = all
        .iter()
        .map(|f| {
            let w = 2.0 * std::f64::consts::PI * f;
            let rot_e = C64::from_polar(1.0, -w * dt);
            let mut pe = C64::from_polar(dt, -w * dt);
            let mut ph = C64::from_polar(dt, -w * 0.5 * dt);
            let (mut v, mut i) = (C64::new(0.0, 0.0), C64::new(0.0, 0.0));
            for pair in series.chunks_exact(2) {
                v += pe * pair[0] as f64;
                i += ph * pair[1] as f64;
                pe *= rot_e;
                ph *= rot_e;
            }
            (v, i)
        })
        .collect();
    let z: Vec<C64> = spectra.iter().map(|(v, i)| v / i).collect();
    let last = z.len() - 1;
    let (v0, i0) = spectra[last];
    let far =
        s.patches.as_ref().filter(|_| !rec.ntff.is_empty()).map(|p| {
            ntff::Box::new(fieldgrid::fdtd::Far::load(p, &rec.ntff, 1, 0, s.f0, (1.0, 0.0)))
        });
    let stats = Stats { cells: s.grid.cells(), steps: rec.steps, dt, decay_db: rec.decay_db, gpu };
    Outcome {
        sweep: Sweep { freqs: s.freqs, z: z[..last].to_vec() },
        z0: z[last],
        p_in: 0.5 * (v0 * i0.conj()).re,
        far,
        stats,
        grid: s.grid,
    }
}

pub fn run(
    model: &Model,
    opts: &Options,
    mut progress: impl FnMut(usize, f64) -> bool,
) -> Result<Outcome, String> {
    let s = setup(model, opts)?;
    let pulse = fieldgrid::fdtd::Pulse { f0: s.f0, fc: s.fc };
    let rec = fieldgrid::fdtd::run_cpu(&s.sim, 0, &pulse, &extras(&s, opts), &mut progress)?;
    Ok(finish(s, rec, false))
}

pub async fn run_gpu(
    model: &Model,
    opts: &Options,
    mut progress: impl FnMut(usize, f64) -> bool,
) -> Result<Outcome, String> {
    let s = setup(model, opts)?;
    let g = gpu::open().await?;
    let pulse = fieldgrid::fdtd::Pulse { f0: s.f0, fc: s.fc };
    let rec = fieldgrid::fdtd::run(&g, &s.sim, 0, &pulse, &extras(&s, opts), &mut progress).await?;
    Ok(finish(s, rec, true))
}
