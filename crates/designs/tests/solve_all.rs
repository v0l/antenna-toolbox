use antenna_designs::matching::plan_match;
use antenna_designs::{DESIGNS, default_controls};
use antenna_solver::solve::{Prepared, segment_cap};
use antenna_solver::units::{C, Unit, format_length};
use std::collections::HashMap;

#[test]
fn every_design_solves_to_something_physical() {
    let freq = 2450.0;
    let lam = C / freq;
    let fmt = |mm: f64| format_length(mm, Unit::Mm);
    let controls = default_controls();
    let overrides = HashMap::new();
    for d in DESIGNS.iter() {
        let comp = d.run(lam, 1.0, &fmt, &overrides, &controls, 1.0);
        let prepared = Prepared::new(&comp.output.solve, lam, 1.0, segment_cap());
        let r = prepared.solve(lam, true);
        let plan = plan_match(d.id, r.z, lam, 50.0);
        eprintln!(
            "{:<12} {:>5} unknowns {:>7.0} ms  {:>6.2} dBi  Z {:>7.1} {:>+8.1}j  AR {:>5.1} dB {}  {}",
            d.id,
            r.segments,
            r.ms,
            r.dbi.unwrap_or(f64::NAN),
            r.z.re,
            r.z.im,
            r.pol.map(|p| p.ar_db).unwrap_or(f64::NAN),
            r.pol.map(|p| p.hand.label()).unwrap_or("-"),
            plan.headline,
        );
        assert!(r.z.re.is_finite() && r.z.re > 0.0, "{} R {}", d.id, r.z.re);
        assert!(r.dbi.unwrap() > -3.0 && r.dbi.unwrap() < 25.0, "{} gain", d.id);
        assert!(!comp.output.diagram.items.is_empty());
    }
}
