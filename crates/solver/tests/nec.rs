mod nec_vectors;

use antenna_solver::C64;
use antenna_solver::geometry::WireGeometry;
use antenna_solver::mom::{
    build_model, far_field_vector, pattern_of, radiated_power, segment_currents, solve_cpu,
};
use nec_vectors::VECTORS;
use std::f64::consts::PI;
use std::sync::Arc;

const LAM: f64 = 1000.0;

fn solve(v: &nec_vectors::Vector) -> (C64, f64) {
    let mut geo = WireGeometry::new(v.lines.iter().map(|l| l.to_vec()).collect(), v.feed);
    if v.ground {
        geo = geo.ground(0.0);
    }
    let m = build_model(&geo, LAM, 2.0 * v.radius, 2000);
    let k = 2.0 * PI / LAM;
    let coeffs = solve_cpu(&m, k);
    let i = coeffs[m.feed];
    let cur = segment_currents(&m, &coeffs);
    let field = far_field_vector(Arc::new(m.segs.clone()), Arc::new(cur), k, m.ground_z, m.images.clone(), None);
    let p_rad = radiated_power(&*pattern_of(field), k);
    let p_in = 0.5 * i.re;
    (i.inv(), p_rad / p_in)
}

#[test]
fn wire_solver_agrees_with_nec2() {
    let mut failed = Vec::new();
    for v in VECTORS {
        let (z, balance) = solve(v);
        let nec = C64::new(v.z.0, v.z.1);
        let err = (z - nec).norm();
        let ok = err < 0.05 * nec.norm() + 3.0;
        eprintln!(
            "{:<12} ours {:>7.1} {:>+7.1}j  nec2 {:>7.1} {:>+7.1}j  |dz| {:>5.1}  power balance {:.3} {}",
            v.name, z.re, z.im, nec.re, nec.im, err, balance, if ok { "" } else { "FAIL" }
        );
        if !ok || (balance - 1.0).abs() > 0.03 {
            failed.push(v.name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
