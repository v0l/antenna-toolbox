use antenna_designs::{by_id, default_controls, dress};
use antenna_solver::geometry::{COPPER, WireProps};
use antenna_solver::solve::{Prepared, fast_sweep, sweep, sweep_cap};
use antenna_solver::units::C;
use std::collections::HashMap;

#[test]
fn a_fast_sweep_follows_the_full_one_with_fewer_solves() {
    let lam = C / 162.0;
    let fmt = |mm: f64| format!("{mm}");
    for id in ["dipole", "moxon", "yagi", "lpda", "jpole", "qfh", "efhw", "discone"] {
        let d = by_id(id);
        let mut out = d.run(lam, 2.0, &fmt, &HashMap::new(), &default_controls(), 1.0).output;
        dress(&mut out.solve, WireProps { conductivity: Some(COPPER), insulation: None });
        let model = Prepared::new(&out.solve, lam, 2.0, sweep_cap());
        let n = 61;
        let full = sweep(&model, 162.0, 0.15, n, |_| {}, || false).unwrap();
        let fast =
            fast_sweep(|f| model.solve(C / f, false).z, 162.0, 0.15, n, 2e-3, |_| {}, || false)
                .unwrap();
        let g = |z: antenna_solver::C64| (z - 50.0) / (z + 50.0);
        let worst = full
            .iter()
            .zip(&fast.points)
            .map(|(a, b)| (g(a.z) - g(b.z)).norm())
            .fold(0.0, f64::max);
        eprintln!("{id:<8} {:>2} solves of {n}, worst |Δγ| {worst:.5}", fast.solves);
        assert!(worst < 5e-3, "{id}: {worst}");
        assert!(fast.solves < n / 2, "{id}: {} solves", fast.solves);
    }
}
