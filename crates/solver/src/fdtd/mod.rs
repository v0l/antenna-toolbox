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
    far: Option<ntff::Box>,
    freqs: Vec<f64>,
    f0: f64,
    fc: f64,
    pulse_end: usize,
    grid: mesh::Grid,
}

fn setup(model: &Model, opts: &Options) -> Result<Setup, String> {
    let port = model.port.ok_or("the model has no port")?;
    let grid = mesh::mesh(model, opts.pml).ok_or("nothing to mesh")?;
    let mats = engine::Materials::from_model(&grid, model);
    let sim = engine::Sim::new(grid.clone(), &mats, &model.metal, Some(port));
    let f0 = model.f0;
    let fc = f0 * 0.5;
    let freqs: Vec<f64> = (0..opts.points)
        .map(|i| {
            f0 * (1.0 - model.span + 2.0 * model.span * i as f64 / (opts.points - 1).max(1) as f64)
        })
        .collect();
    let far = opts.far_field.then(|| ntff::Box::new(&sim, 3, f0));
    let (t0, _) = engine::pulse_shape(fc);
    let pulse_end = (2.0 * t0 / sim.dt) as usize;
    Ok(Setup { sim, far, freqs, f0, fc, pulse_end, grid })
}

fn finish(s: Setup, series: &[f32], stats: Stats) -> Outcome {
    let dt = s.sim.dt;
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
    Outcome {
        sweep: Sweep { freqs: s.freqs, z: z[..last].to_vec() },
        z0: z[last],
        p_in: 0.5 * (v0 * i0.conj()).re,
        far: s.far,
        stats,
        grid: s.grid,
    }
}

pub fn run(
    model: &Model,
    opts: &Options,
    mut progress: impl FnMut(usize, f64) -> bool,
) -> Result<Outcome, String> {
    let mut s = setup(model, opts)?;
    let pulse = engine::gaussian(s.f0, s.fc);
    let dt = s.sim.dt;
    let mut series: Vec<f32> = Vec::new();
    let mut peak: f64 = 0.0;
    let mut decay = 0.0;
    let mut steps = 0;
    for n in 0..opts.max_steps {
        s.sim.update_h();
        let th = (n as f64 + 0.5) * dt;
        let i = s.sim.port_current();
        s.sim.update_e(pulse(th) as f32);
        let te = (n as f64 + 1.0) * dt;
        series.push(s.sim.port_voltage() as f32);
        series.push(i as f32);
        if let Some(b) = s.far.as_mut() {
            b.accumulate(&s.sim, te, th);
        }
        steps = n + 1;
        if n % 200 == 199 {
            let e = s.sim.energy();
            peak = peak.max(e);
            decay = if e > 0.0 && peak > 0.0 { 10.0 * (peak / e).log10() } else { 0.0 };
            if !progress(n, decay) {
                return Err("cancelled".into());
            }
            if n > s.pulse_end && decay >= opts.decay_db {
                break;
            }
        }
    }
    let stats = Stats { cells: s.grid.cells(), steps, dt, decay_db: decay, gpu: false };
    Ok(finish(s, &series, stats))
}

pub async fn run_gpu(
    model: &Model,
    opts: &Options,
    mut progress: impl FnMut(usize, f64) -> bool,
) -> Result<Outcome, String> {
    let mut s = setup(model, opts)?;
    let g = gpu::GpuFdtd::new(&s.sim, s.far.as_ref(), s.f0, s.fc, opts.max_steps).await?;
    let chunk = 500;
    let mut steps = 0;
    let mut peak: f64 = 0.0;
    let mut decay = 0.0;
    while steps < opts.max_steps {
        let n = chunk.min(opts.max_steps - steps);
        g.advance(n);
        steps += n;
        let e = g.energy().await?;
        peak = peak.max(e);
        decay = if e > 0.0 && peak > 0.0 { 10.0 * (peak / e).log10() } else { 0.0 };
        if !e.is_finite() {
            return Err("the GPU run went unstable".into());
        }
        if !progress(steps, decay) {
            return Err("cancelled".into());
        }
        if steps > s.pulse_end && decay >= opts.decay_db {
            break;
        }
    }
    let series = g.series(0, steps).await?;
    if let Some(b) = s.far.as_mut() {
        b.load(&g.far_field().await?);
    }
    let stats = Stats { cells: s.grid.cells(), steps, dt: s.sim.dt, decay_db: decay, gpu: true };
    Ok(finish(s, &series, stats))
}
