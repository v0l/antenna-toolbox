mod nec_vectors;

use antenna_solver::C64;
use antenna_solver::geometry::{RealGround, WireGeometry, WireProps};
use antenna_solver::mom::{
    build_model, far_field_vector, pattern_of, radiated_power, segment_currents, solve_cpu,
};
use antenna_solver::solve::{model_for, solve_at};
use nec_vectors::VECTORS;
use std::f64::consts::PI;
use std::sync::Arc;

const LAM: f64 = 1000.0;

fn geometry(v: &nec_vectors::Vector) -> WireGeometry {
    let mut geo = WireGeometry::new(v.lines.iter().map(|l| l.to_vec()).collect(), v.feed)
        .with_props(WireProps { conductivity: v.sigma, insulation: None });
    if v.ground {
        geo = geo.ground(0.0);
    }
    geo.real_ground = v.real.map(|(eps_r, sigma)| RealGround { eps_r, sigma });
    geo
}

fn solve(v: &nec_vectors::Vector) -> (C64, f64) {
    let geo = geometry(v);
    if v.real.is_some() || v.sigma.is_some() {
        return (C64::new(0.0, 0.0), 1.0);
    }
    let m = build_model(&geo, LAM, 2.0 * v.radius, 2000);
    let k = 2.0 * PI / LAM;
    let coeffs = solve_cpu(&m, k);
    let i = coeffs[m.feed];
    let cur = segment_currents(&m, &coeffs);
    let field = far_field_vector(
        Arc::new(m.segs.clone()),
        Arc::new(cur),
        k,
        m.ground_z,
        m.images.clone(),
        None,
        None,
    );
    let p_rad = radiated_power(&*pattern_of(field), k);
    let p_in = 0.5 * i.re;
    (i.inv(), p_rad / p_in)
}

#[test]
fn loss_and_real_ground_agree_with_nec2() {
    let mut failed = Vec::new();
    for v in VECTORS {
        let r = solve_at(&model_for(&geometry(v), LAM, 2.0 * v.radius, 2000), LAM, true);
        let nec = C64::new(v.z.0, v.z.1);
        let err = (r.z - nec).norm();
        let dg = r.dbi.unwrap() - v.gain;
        let ok = err < 0.05 * nec.norm() + 3.0
            && (v.sigma.is_none() || (r.z.re - nec.re).abs() < 0.1 * nec.re + 0.3)
            && dg.abs() < 0.35;
        eprintln!(
            "{:<36} ours {:>7.2} {:>+8.2}j {:>6.2} dBi  nec2 {:>7.2} {:>+8.2}j {:>6.2} dBi  eff {:.3} {}",
            v.name,
            r.z.re,
            r.z.im,
            r.dbi.unwrap(),
            nec.re,
            nec.im,
            v.gain,
            r.efficiency.unwrap_or(1.0),
            if ok { "" } else { "FAIL" }
        );
        if !ok {
            failed.push(v.name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

#[test]
fn wire_solver_agrees_with_nec2() {
    let mut failed = Vec::new();
    for v in VECTORS.iter().filter(|v| v.real.is_none() && v.sigma.is_none()) {
        let (z, balance) = solve(v);
        let nec = C64::new(v.z.0, v.z.1);
        let err = (z - nec).norm();
        let ok = err < 0.05 * nec.norm() + 3.0;
        eprintln!(
            "{:<12} ours {:>7.1} {:>+7.1}j  nec2 {:>7.1} {:>+7.1}j  |dz| {:>5.1}  power balance {:.3} {}",
            v.name,
            z.re,
            z.im,
            nec.re,
            nec.im,
            err,
            balance,
            if ok { "" } else { "FAIL" }
        );
        if !ok || (balance - 1.0).abs() > 0.03 {
            failed.push(v.name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
