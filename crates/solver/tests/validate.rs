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
    let geo =
        WireGeometry::new(vec![vec![[-len / 2.0, 0.0, 0.0], [len / 2.0, 0.0, 0.0]]], [0.0; 3]);
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

fn resonant_half_length(props: antenna_solver::geometry::WireProps) -> f64 {
    let lam = 299_792.458 / 30.0;
    let x = |half: f64| {
        let g = WireGeometry::new(vec![vec![[-half, 0.0, 0.0], [half, 0.0, 0.0]]], [0.0; 3])
            .with_props(props);
        solve_at(&model_for(&g, lam, 2.0, 900), lam, false).z.im
    };
    let (mut lo, mut hi) = (2000.0, 2600.0);
    for _ in 0..30 {
        let m = (lo + hi) / 2.0;
        if x(m) < 0.0 { lo = m } else { hi = m }
    }
    lo
}

#[test]
fn insulated_dipole_matches_the_nec4_is_card() {
    use antenna_solver::geometry::{COPPER, Insulation, WireProps};
    let bare = resonant_half_length(WireProps { conductivity: Some(COPPER), insulation: None });
    let sheathed = resonant_half_length(WireProps {
        conductivity: Some(COPPER),
        insulation: Some(Insulation { eps_r: 2.25, inner: 1.0, outer: 3.0 }),
    });
    assert!((bare - 2416.0).abs() < 8.0, "bare {bare}");
    assert!((sheathed - 2302.0).abs() < 8.0, "sheathed {sheathed}");
}

#[test]
fn a_lumped_load_adds_in_series_at_the_feed() {
    let deck = "GW 1 7 0 0 -.25 0 0 .25 .001\nGE\nFR 0 1 0 0 299.8 0\nEX 0 1 4 0 1.\n";
    let solve = |text: &str| {
        let r = antenna_solver::nec::import(text).unwrap();
        let lam = 299_792.458 / r.freq_mhz.unwrap();
        solve_at(&model_for(&r.geo, lam, 2.0, 900), lam, false).z
    };
    let bare = solve(&format!("{deck}EN\n"));
    let loaded = solve(&format!("{deck}LD 0 1 4 4 10. 3.000E-09 5.300E-11\nEN\n"));
    let d = loaded - bare;
    assert!((d.re - 10.0).abs() < 0.05 && (d.im + 4.36).abs() < 0.05, "{d}");
}

#[test]
fn a_second_source_drives_a_phased_pair() {
    let deck = "GW 1 21 0 -0.24 0 0 0.24 0 0.001\nGW 2 21 0.25 -0.24 0 0.25 0.24 0 0.001\nGE\nFR 0 1 0 0 299.8 0\nEX 0 1 11 0 1 0\nEX 0 2 11 0 0 -1\nEN\n";
    let r = antenna_solver::nec::import(deck).unwrap();
    assert_eq!(r.geo.sources.len(), 1);
    assert_eq!(r.geo.sources[0].1, C64::new(0.0, -1.0));
    let lam = 299_792.458 / 299.8;
    let res = solve_at(&model_for(&r.geo, lam, 2.0, 900), lam, true);
    let fwd = res.gain_dbi([1.0, 0.0, 0.0]).unwrap();
    let back = res.gain_dbi([-1.0, 0.0, 0.0]).unwrap();
    assert!(
        (fwd - 5.48).abs() < 0.2 && (back - 1.95).abs() < 0.2,
        "nec2 5.48 / 1.95, ours {fwd} / {back}"
    );
    assert!((res.z - C64::new(52.06, 14.13)).norm() < 3.0, "nec2 52.06+14.13j, ours {}", res.z);
}
