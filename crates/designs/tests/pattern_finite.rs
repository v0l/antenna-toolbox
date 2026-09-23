use antenna_designs::{DESIGNS, default_controls};
use antenna_solver::solve::{Prepared, segment_cap};
use antenna_solver::units::C;
use std::collections::HashMap;

#[test]
fn every_pattern_is_finite_everywhere() {
    let lam = C / 162.0;
    let fmt = |mm: f64| format!("{mm}");
    for d in DESIGNS.iter() {
        let comp = d.run(lam, 2.0, &fmt, &HashMap::new(), &default_controls(), 1.0);
        let r = Prepared::new(&comp.output.solve, lam, 2.0, segment_cap()).solve(lam, true);
        let p = r.pattern.unwrap();
        let mut bad = 0;
        for i in 0..=30 {
            let th = i as f64 / 30.0 * std::f64::consts::PI;
            for j in 0..=40 {
                let ph = j as f64 / 40.0 * std::f64::consts::TAU;
                let v = p([th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()]);
                if !v.is_finite() {
                    bad += 1;
                }
            }
        }
        assert_eq!(bad, 0, "{} has {bad} non-finite directions", d.id);
    }
}
