use crate::geometry::{Geometry, WireGeometry};
use crate::gpu;
use crate::mom::{
    Directivity, FieldFn, Model, Pattern, build_model, directivity, far_field_vector, pattern_of,
    segment_currents, solve_cpu,
};
use crate::po::{hybrid_field, po_currents};
use crate::polarisation::{Ellipse, ellipse_at};
use crate::surface::{SurfaceModel, solve_surface, surface_model};
use crate::units::C;
use crate::vec::Vec3;
use num_complex::Complex64 as C64;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Gpu,
    Cpu,
    CpuFallback,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Gpu => "GPU",
            Backend::Cpu => "CPU",
            Backend::CpuFallback => "CPU fallback",
        }
    }
}

#[derive(Clone)]
pub struct SolveResult {
    pub how: Backend,
    pub z: C64,
    pub swr: f64,
    pub ms: f64,
    pub segments: usize,
    pub max_degree: usize,
    pub pattern: Option<Pattern>,
    pub field: Option<FieldFn>,
    pub dbi: Option<f64>,
    pub peak: f64,
    pub pol: Option<Ellipse>,
    pub hybrid: bool,
}

impl SolveResult {
    pub fn gain_dbi(&self, dir: Vec3) -> Option<f64> {
        let (p, dbi) = (self.pattern.as_ref()?, self.dbi?);
        let v = p(dir);
        Some(dbi + 20.0 * (v.max(1e-12) / self.peak.max(1e-30)).log10())
    }

    pub fn junction(&self) -> bool {
        self.max_degree > 2
    }
}

pub fn segment_cap() -> usize {
    900
}

pub fn sweep_cap() -> usize {
    if gpu::ready() { 420 } else { 300 }
}

pub fn swr_of(z: C64, z0: f64) -> f64 {
    let g = (z - z0).norm() / (z + z0).norm().max(1e-12);
    ((1.0 + g) / (1.0 - g).max(1e-6)).min(99.0)
}

pub fn model_for(geo: &WireGeometry, lam: f64, wire_dia: f64, cap: usize) -> Model {
    build_model(geo, lam, wire_dia, cap)
}

pub fn solve_at(model: &Model, lam: f64, want_pattern: bool) -> SolveResult {
    let t0 = Instant::now();
    let k = 2.0 * PI / lam;
    let coeffs = solve_cpu(model, k);
    let how = Backend::Cpu;
    let i = coeffs.get(model.feed).copied().unwrap_or_default();
    let z = if i.norm_sqr() > 0.0 { i.inv() } else { C64::new(1e30, 0.0) };
    let currents = segment_currents(model, &coeffs);

    let mut result = SolveResult {
        how,
        z,
        swr: swr_of(z, 50.0),
        ms: 0.0,
        segments: model.segs.len(),
        max_degree: model.max_degree,
        pattern: None,
        field: None,
        dbi: None,
        peak: 0.0,
        pol: None,
        hybrid: false,
    };

    if want_pattern {
        let segs = Arc::new(model.segs.clone());
        let cur = Arc::new(currents);
        let field = match &model.po {
            Some(mesh) => {
                let facets = po_currents(mesh, &segs, &cur, k, model.feed_point);
                result.hybrid = true;
                hybrid_field(segs.clone(), cur.clone(), Arc::new(facets), k)
            }
            None => far_field_vector(
                segs.clone(),
                cur.clone(),
                k,
                model.ground_z,
                model.images.clone(),
                model.blocked.clone(),
            ),
        };
        let pattern = pattern_of(field.clone());
        let Directivity { linear, peak } = directivity(&*pattern);
        result.dbi = Some(10.0 * linear.max(1e-6).log10());
        result.peak = peak;
        if model.po.is_none() {
            result.pol = Some(ellipse_at(&field, peak_direction(&*pattern)));
        }
        result.pattern = Some(pattern);
        result.field = Some(field);
    }
    result.ms = t0.elapsed().as_secs_f64() * 1e3;
    result
}

pub fn peak_direction(pattern: &(dyn Fn(Vec3) -> f64 + Sync)) -> Vec3 {
    let mut best = f64::NEG_INFINITY;
    let mut dir = [0.0, 0.0, 1.0];
    for i in 0..=36 {
        let th = i as f64 / 36.0 * PI;
        for j in 0..72 {
            let ph = j as f64 / 72.0 * 2.0 * PI;
            let d = [th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()];
            let v = pattern(d);
            if v > best {
                best = v;
                dir = d;
            }
        }
    }
    dir
}

pub enum Prepared {
    Wire(Model),
    Surface(SurfaceModel),
}

impl Prepared {
    pub fn new(geo: &Geometry, lam: f64, wire_dia: f64, cap: usize) -> Self {
        match geo {
            Geometry::Wire(w) => Prepared::Wire(model_for(w, lam, wire_dia, cap)),
            Geometry::Surface(s) => Prepared::Surface(surface_model(s)),
        }
    }

    pub fn solve(&self, lam: f64, want_pattern: bool) -> SolveResult {
        match self {
            Prepared::Wire(m) => solve_at(m, lam, want_pattern),
            Prepared::Surface(m) => solve_surface(m, lam, want_pattern),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SweepPoint {
    pub f: f64,
    pub z: C64,
}

pub fn sweep(
    model: &Prepared,
    f0: f64,
    span_fraction: f64,
    points: usize,
    mut on_point: impl FnMut(&[SweepPoint]),
    cancelled: impl Fn() -> bool,
) -> Option<Vec<SweepPoint>> {
    let mut out = Vec::with_capacity(points);
    for i in 0..points {
        if cancelled() {
            return None;
        }
        let f = f0 * (1.0 - span_fraction + 2.0 * span_fraction * i as f64 / (points - 1) as f64);
        let r = model.solve(C / f, false);
        out.push(SweepPoint { f, z: r.z });
        on_point(&out);
    }
    Some(out)
}

pub struct Tuned {
    pub scale: f64,
    pub z: C64,
}

pub fn tune_to_resonance(
    mut solve_scaled: impl FnMut(f64) -> C64,
    mut on_step: impl FnMut(f64, f64),
) -> Option<Tuned> {
    let mut at = |s: f64| {
        let z = solve_scaled(s);
        on_step(s, z.im);
        z
    };
    const LO: f64 = 0.7;
    const HI: f64 = 1.35;
    const STEPS: usize = 14;
    let scales: Vec<f64> = (0..=STEPS).map(|i| LO + (HI - LO) * i as f64 / STEPS as f64).collect();
    let samples: Vec<C64> = scales.iter().map(|&s| at(s)).collect();

    let mut brackets: Vec<(f64, f64)> = (1..=STEPS)
        .filter(|&i| {
            let (a, b) = (samples[i - 1], samples[i]);
            a.im < 0.0 && b.im >= 0.0 && a.re < 600.0 && b.re < 600.0
        })
        .map(|i| (scales[i - 1], scales[i]))
        .collect();
    if brackets.is_empty() {
        return None;
    }
    brackets.sort_by(|x, y| {
        ((x.0 + x.1) / 2.0 - 1.0).abs().total_cmp(&((y.0 + y.1) / 2.0 - 1.0).abs())
    });
    let (mut lo, mut hi) = brackets[0];
    let mut r_lo = at(lo);
    let mut r_hi = at(hi);
    for _ in 0..10 {
        if hi - lo <= 0.0015 {
            break;
        }
        let mid = (lo + hi) / 2.0;
        let r = at(mid);
        if r.im < 0.0 {
            lo = mid;
            r_lo = r;
        } else {
            hi = mid;
            r_hi = r;
        }
    }
    Some(if r_lo.im.abs() < r_hi.im.abs() {
        Tuned { scale: lo, z: r_lo }
    } else {
        Tuned { scale: hi, z: r_hi }
    })
}
