pub mod mesh;
pub mod rwg;

use crate::geometry::SurfaceGeometry;
use crate::gpu;
use crate::linalg::solve_system;
use crate::mom::{Directivity, directivity, pattern_of};
use crate::solve::{Backend, SolveResult, swr_of};
use mesh::{SurfaceTopology, feed_edges, topology};
use rwg::{fill_surface, surface_field, surface_impedance};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct SurfaceModel {
    pub topo: SurfaceTopology,
    pub feed: Vec<usize>,
}

pub fn surface_model(geo: &SurfaceGeometry) -> SurfaceModel {
    let topo = topology(&geo.mesh);
    let feed = feed_edges(&topo, geo.feed, geo.feed_dir, geo.feed_tol);
    SurfaceModel { topo, feed }
}

pub fn solve_surface(model: &SurfaceModel, lam: f64, want_pattern: bool) -> SolveResult {
    let t0 = Instant::now();
    let k = 2.0 * std::f64::consts::PI / lam;

    let mut cells: Vec<f64> = model.topo.triangles.iter().map(|t| t.area.sqrt()).collect();
    cells.sort_by(f64::total_cmp);
    let median = cells.get(cells.len() / 2).copied().unwrap_or(0.0);
    let fine_mesh = median < lam / 200.0;

    let cpu = || solve_system(fill_surface(&model.topo, k, &model.feed));
    let (cur, how) = if gpu::ready() && !fine_mesh {
        match gpu::fill_surface(&model.topo, k, &model.feed) {
            Ok(sys) => {
                let cur = solve_system(sys);
                let mag = cur[model.feed[0]].norm();
                if mag.is_finite() && mag > 0.0 {
                    (cur, Backend::Gpu)
                } else {
                    (cpu(), Backend::CpuFallback)
                }
            }
            Err(_) => (cpu(), Backend::CpuFallback),
        }
    } else {
        (cpu(), Backend::Cpu)
    };
    let z = surface_impedance(&model.topo.edges, &model.feed, &cur);

    let mut result = SolveResult {
        how,
        z,
        swr: swr_of(z, 50.0),
        ms: t0.elapsed().as_secs_f64() * 1e3,
        segments: model.topo.edges.len(),
        max_degree: 2,
        pattern: None,
        field: None,
        dbi: None,
        directivity: None,
        efficiency: None,
        peak: 0.0,
        pol: None,
        hybrid: false,
    };
    if want_pattern {
        let field = surface_field(&model.topo, Arc::new(cur), k);
        let pattern = pattern_of(field.clone());
        let Directivity { linear, peak } = directivity(&*pattern);
        result.dbi = Some(10.0 * linear.max(1e-6).log10());
        result.directivity = result.dbi;
        result.peak = peak;
        result.pattern = Some(pattern);
        result.field = Some(field);
    }
    result
}
