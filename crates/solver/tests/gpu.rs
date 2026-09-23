use antenna_solver::gpu;
use antenna_solver::linalg::solve_system;

use antenna_solver::surface::mesh::{feed_edges, merge_meshes, mesh_rect, topology};
use antenna_solver::surface::rwg::{fill_surface, surface_impedance};
use antenna_solver::units::C;
use std::f64::consts::PI;

const LAM: f64 = C / 300.0;

#[test]
fn gpu_surface_fill_agrees_with_f64_cpu() {
    if gpu::init().is_none() {
        return;
    }
    let (hl, hw, ax, cy) = (0.475 * LAM / 2.0, LAM / 200.0, LAM / 50.0, LAM / 300.0);
    let mesh = merge_meshes(
        &[mesh_rect(-hl, 0.0, -hw, hw, 0.0, ax, cy), mesh_rect(0.0, hl, -hw, hw, 0.0, ax, cy)],
        ax.min(cy) / 100.0,
    );
    let topo = topology(&mesh);
    let feed = feed_edges(&topo, [0.0; 3], [1.0, 0.0, 0.0], ax * 0.2);
    let k = 2.0 * PI / LAM;
    let cpu = surface_impedance(&topo.edges, &feed, &solve_system(fill_surface(&topo, k, &feed)));
    let sys = gpu::fill_surface(&topo, k, &feed).unwrap();
    let g = surface_impedance(&topo.edges, &feed, &solve_system(sys));
    eprintln!("cpu {cpu} gpu {g}");
    assert!((cpu - g).norm() < 1.0, "cpu {cpu} gpu {g}");
}
