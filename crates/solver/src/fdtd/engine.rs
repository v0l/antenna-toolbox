use super::mesh::Grid;
use super::{Aabb, EPS0, Model, Port, Stuff};
use sims_field::Bvh;
use sims_field::fdtd::{Edge, Materials, Media, PortDef};
use sims_field::raster::{closed, sheet_edges, solid_cells, solid_edges};

pub use sims_field::fdtd::{Sim, gaussian, idx, pulse_shape, time_step};

pub fn materials(grid: &Grid, model: &Model) -> Materials {
    let [_, ny, nz] = grid.dims();
    let mut mats = Materials::new(grid);
    let w = 2.0 * std::f64::consts::PI * model.f0;
    for d in &model.dielectrics {
        let range = |a: usize| {
            let ax = grid.axis(a);
            let lo = ax.partition_point(|v| *v < d.bounds.lo[a] - 1e-9);
            let hi = ax.partition_point(|v| *v <= d.bounds.hi[a] + 1e-9);
            lo..hi.saturating_sub(1).max(lo)
        };
        for i in range(0) {
            for j in range(1) {
                for k in range(2) {
                    let c = (i * (ny - 1) + j) * (nz - 1) + k;
                    mats.eps[c] = d.eps_r as f32;
                    mats.inf[c] = d.eps_r as f32;
                    mats.sigma[c] = (w * EPS0 * d.eps_r * d.tan_d) as f32;
                }
            }
        }
    }
    for part in &model.parts {
        let Stuff::Dielectric { eps_r, tan_d } = part.stuff else { continue };
        let inside = solid_cells(&Bvh::new(part.triangles.clone()), grid);
        for (c, _) in inside.iter().enumerate().filter(|(_, v)| **v) {
            mats.eps[c] = eps_r as f32;
            mats.inf[c] = eps_r as f32;
            mats.sigma[c] = (w * EPS0 * eps_r * tan_d) as f32;
        }
    }
    mats
}

fn metal_edges(grid: &Grid, metal: &[Aabb]) -> [Vec<bool>; 3] {
    let n = grid.dims();
    let total = n[0] * n[1] * n[2];
    let mut mask = [vec![false; total], vec![false; total], vec![false; total]];
    for m in metal {
        let range = |a: usize| {
            let axv = grid.axis(a);
            let lo = axv.partition_point(|v| *v < m.lo[a] - 1e-9);
            let hi = axv.partition_point(|v| *v <= m.hi[a] + 1e-9);
            (lo, hi)
        };
        let r = [range(0), range(1), range(2)];
        for (c, edges) in mask.iter_mut().enumerate() {
            let mut lim = r;
            lim[c].1 = lim[c].1.saturating_sub(1);
            for i in lim[0].0..lim[0].1 {
                for j in lim[1].0..lim[1].1 {
                    for k in lim[2].0..lim[2].1 {
                        edges[idx(n, i, j, k)] = true;
                    }
                }
            }
        }
    }
    mask
}

fn port_def(grid: &Grid, p: Port) -> PortDef {
    let axis = (0..3)
        .max_by(|a, b| (p.b[*a] - p.a[*a]).abs().total_cmp(&(p.b[*b] - p.a[*b]).abs()))
        .unwrap_or(2);
    let at = [0, 1, 2].map(|t| grid.nearest(t, p.a[t].min(p.b[t])));
    let end = grid.nearest(axis, p.a[axis].max(p.b[axis]));
    let count = end.saturating_sub(at[axis]).max(1);
    let edges = (0..count)
        .map(|s| {
            let mut q = at;
            q[axis] += s;
            Edge { comp: axis, at: q }
        })
        .collect();
    PortDef { name: "feed".into(), columns: vec![edges], r: p.r, reference_above: false }
}

pub fn build(grid: Grid, model: &Model, port: Port) -> Sim {
    let mats = materials(&grid, model);
    let mut mask = metal_edges(&grid, &model.metal);
    for part in model.parts.iter().filter(|p| p.stuff == Stuff::Metal) {
        let mut add = |more: [Vec<bool>; 3]| {
            for (edges, extra) in mask.iter_mut().zip(more) {
                edges.iter_mut().zip(extra).for_each(|(e, x)| *e |= x);
            }
        };
        add(sheet_edges(&part.triangles, &grid));
        if closed(&part.triangles) {
            add(solid_edges(&Bvh::new(part.triangles.clone()), &grid));
        }
    }
    let n = grid.dims();
    let pec = |c: usize, p: [usize; 3]| mask[c][idx(n, p[0], p[1], p[2])];
    let ports = vec![port_def(&grid, port)];
    Sim::new(grid, &mats, &pec, &[], ports, &[], Media::default())
}
