use antenna_solver::C64;
use antenna_solver::geometry::WireGeometry;
use antenna_solver::linalg::solve_system;
use antenna_solver::mom::{build_model, pattern_of, radiated_power, solve_cpu};
use antenna_solver::solve::{model_for, solve_at};
use antenna_solver::surface::mesh::{feed_edges, merge_meshes, mesh_rect, topology};
use antenna_solver::surface::rwg::{fill_surface, surface_field, surface_impedance};
use std::f64::consts::PI;
use std::sync::Arc;

const LAM: f64 = 1000.0;
const K: f64 = 2.0 * PI / LAM;

fn dipole(len: f64, radius: f64) -> C64 {
    let geo = WireGeometry::new(vec![vec![[-len / 2.0, 0.0, 0.0], [len / 2.0, 0.0, 0.0]]], [0.0; 3]);
    let mut m = build_model(&geo, LAM, 2.0 * radius, 10_000);
    m.a = radius;
    solve_cpu(&m, K)[m.feed].inv()
}

#[test]
fn dipole_matches_king_middleton_second_order() {
    let table = [
        (1.2, 10.0, 37.842, -127.001),
        (1.4, 10.0, 59.153, -34.307),
        (1.6, 10.0, 91.403, 54.582),
        (1.2, 12.5, 37.134, -185.797),
        (1.4, 12.5, 57.384, -60.07),
        (1.6, 12.5, 87.655, 60.400),
        (1.2, 15.0, 36.742, -244.013),
        (1.4, 15.0, 56.36, -86.012),
        (1.6, 15.0, 85.404, 65.397),
        (1.2, 20.0, 36.336, -361.567),
        (1.4, 20.0, 55.248, -138.041),
        (1.6, 20.0, 82.88, 74.476),
    ];
    for (bh, omega, r, x) in table {
        let h = bh / K;
        let z = dipole(2.0 * h, 2.0 * h / (omega / 2.0f64).exp());
        let km = C64::new(r, x);
        assert!((z - km).norm() < 0.06 * km.norm() + 4.0, "βh {bh} Ω {omega}: {z} vs {km}");
    }
}

#[test]
fn strip_converges_to_its_equivalent_wire() {
    let (len, width, axial, across) = (475.0, 10.0, 5.0, 6);
    let (hl, hw, cy) = (len / 2.0, width / 2.0, width / across as f64);
    let mesh = merge_meshes(
        &[
            mesh_rect(-hl, 0.0, -hw, hw, 0.0, axial, cy),
            mesh_rect(0.0, hl, -hw, hw, 0.0, axial, cy),
        ],
        axial.min(cy) / 100.0,
    );
    let topo = topology(&mesh);
    let feed = feed_edges(&topo, [0.0; 3], [1.0, 0.0, 0.0], axial * 0.2);
    let cur = solve_system(fill_surface(&topo, K, &feed));
    let z = surface_impedance(&topo.edges, &feed, &cur);
    let wire = dipole(len, width / 4.0);
    assert!((z - wire).norm() < 2.0, "strip {z} wire {wire}");

    let p_in = 0.5 * z.inv().re;
    let p_rad = radiated_power(&*pattern_of(surface_field(&topo, Arc::new(cur), K)), K);
    assert!((p_rad / p_in - 1.0).abs() < 0.01, "power balance {}", p_rad / p_in);
}

#[test]
fn physical_optics_plate_approaches_image_theory() {
    let dip = || {
        WireGeometry::new(vec![vec![[-235.0, 0.0, 250.0], [235.0, 0.0, 250.0]]], [0.0, 0.0, 250.0])
    };
    let image = solve_at(&model_for(&dip().ground(0.0), LAM, 1.0, 900), LAM, true).dbi.unwrap();
    let h = 4.0 * LAM;
    let mut g = dip();
    g.po = Some(mesh_rect(-h, h, -h, h, 0.0, LAM / 8.0, LAM / 8.0));
    let po = solve_at(&model_for(&g, LAM, 1.0, 900), LAM, true).dbi.unwrap();
    assert!((po - image).abs() < 0.1, "PO {po} dBi, image {image} dBi");
    assert!((image - 7.46).abs() < 0.1, "image {image}");
}
