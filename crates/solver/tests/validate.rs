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
        insulation: Some(Insulation { eps_r: 2.25, tan_d: 0.0, inner: 1.0, outer: 3.0 }),
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

fn nec_case(deck: &str) -> (C64, f64, f64) {
    let r = antenna_solver::nec::import(deck).unwrap();
    let lam = 299_792.458 / r.freq_mhz.unwrap();
    let res = solve_at(&model_for(&r.geo, lam, 2.0, 900), lam, true);
    (res.z, res.gain_dbi([1.0, 0.0, 0.0]).unwrap(), res.gain_dbi([-1.0, 0.0, 0.0]).unwrap())
}

#[test]
fn a_crossed_line_between_dipoles_matches_nec2() {
    let (z, fwd, back) = nec_case(
        "GW 1 21 0 -0.24 0 0 0.24 0 0.001\nGW 2 21 0.125 -0.24 0 0.125 0.24 0 0.001\nGE\nFR 0 1 0 0 299.8 0\nEX 0 1 11 0 1 0\nTL 1 11 2 11 -50 0.125 0 0 0 0\nEN\n",
    );
    assert!((z - C64::new(7.38, 18.31)).norm() < 2.5, "nec2 7.38+18.31j, ours {z}");
    assert!(
        (fwd - 1.77).abs() < 0.3 && (back - 7.28).abs() < 0.3,
        "nec2 1.77 / 7.28, ours {fwd} / {back}"
    );
}

#[test]
fn a_general_network_matches_nec2() {
    let (z, fwd, back) = nec_case(
        "GW 1 21 0 -0.24 0 0 0.24 0 0.001\nGW 2 21 0.25 -0.24 0 0.25 0.24 0 0.001\nGE\nFR 0 1 0 0 299.8 0\nEX 0 1 11 0 1 0\nNT 1 11 2 11 0.01 -0.005 0 0.004 0.002 0.001\nEN\n",
    );
    assert!((z - C64::new(33.57, 5.39)).norm() < 2.0, "nec2 33.57+5.39j, ours {z}");
    assert!(
        (fwd - 1.98).abs() < 0.3 && (back + 1.86).abs() < 0.3,
        "nec2 1.98 / -1.86, ours {fwd} / {back}"
    );
}

#[test]
fn nec_example_five_log_periodic_matches_nec2() {
    let deck = "GW 1 5 0.0000 -1.0000 0 0 1.0000 0 .00667
GW 2 5 -.7527 -1.0753 0 -.7527 1.0753 0 .00717
GW 3 5 -1.562 -1.1562 0 -1.562 1.1562 0 .00771
GW 4 5 -2.4323 -1.2432 0 -2.4323 1.2432 0 .00829
GW 5 5 -3.368 -1.3368 0 -3.368 1.3368 0 .00891
GW 6 7 -4.3742 -1.4374 0 -4.3742 1.4374 0 .00958
GW 7 7 -5.4562 -1.5456 0 -5.4562 1.5456 0 .0103
GW 8 7 -6.6195 -1.6619 0 -6.6195 1.6619 0 .01108
GW 9 7 -7.8705 -1.787 0 -7.8705 1.787 0 .01191
GW 10 7 -9.2156 -1.9215 0 -9.2156 1.9215 0 .01281
GW 11 9 -10.6619 -2.0662 0 -10.6619 2.0662 0 .01377
GW 12 9 -12.2171 -2.2217 0 -12.2171 2.2217 0 .01481
GE
FR 0 0 0 0 46.29 0.
TL 1 3 2 3 -50.
TL 2 3 3 3 -50.
TL 3 3 4 3 -50.
TL 4 3 5 3 -50.
TL 5 3 6 4 -50.
TL 6 4 7 4 -50.
TL 7 4 8 4 -50.
TL 8 4 9 4 -50.
TL 9 4 10 4 -50.
TL 10 4 11 5 -50.
TL 11 5 12 5 -50. ,0.,0.,0.,.02
EX 0 1 3 10 1
EN
";
    let r = antenna_solver::nec::import(deck).unwrap();
    let lam = 299_792.458 / r.freq_mhz.unwrap();
    let res = solve_at(&model_for(&r.geo, lam, 2.0, 900), lam, true);
    assert!((res.z - C64::new(42.33, -0.45)).norm() < 1.5, "nec2 42.33-0.45j, ours {}", res.z);
    let eff = res.efficiency.unwrap();
    assert!((eff - 0.9111).abs() < 0.01, "nec2 91.11 % into the termination, ours {eff}");
    let d = res.directivity.unwrap();
    assert!((d - 9.75).abs() < 0.15, "nec2 9.75 dBi directive gain, ours {d}");
}

#[test]
fn a_lossy_sleeve_costs_efficiency_and_a_lossless_one_does_not() {
    use antenna_solver::geometry::{Insulation, WireGeometry, WireProps};
    let lam = 299_792.458 / 162.0;
    let eff = |tan_d: f64| {
        let props = WireProps {
            conductivity: None,
            insulation: Some(Insulation { eps_r: 3.0, tan_d, inner: 1.0, outer: 1.5 }),
        };
        let g = WireGeometry::new(vec![vec![[0.0, -430.0, 0.0], [0.0, 430.0, 0.0]]], [0.0; 3])
            .with_props(props);
        solve_at(&model_for(&g, lam, 2.0, 900), lam, true).efficiency.unwrap()
    };
    let (none, pvc, worse) = (eff(0.0), eff(0.01), eff(0.05));
    assert!(none > 0.998, "{none}");
    assert!(pvc < none && pvc > 0.95, "{pvc}");
    assert!(worse < pvc, "{worse}");
}

#[test]
fn segments_fatter_than_they_are_long_are_counted() {
    let solve = |radius: f64| {
        let deck = format!(
            "GW 1 21 0 -0.24 0 0 0.24 0 {radius}\nGE\nFR 0 1 0 0 299.8 0\nEX 0 1 11 0 1 0\nEN\n"
        );
        let r = antenna_solver::nec::import(&deck).unwrap();
        let lam = 299_792.458 / 299.8;
        solve_at(&model_for(&r.geo, lam, 2.0, 900), lam, false).stubby
    };
    assert_eq!(solve(0.001), 0);
    assert!(solve(0.02) >= 21);
}
