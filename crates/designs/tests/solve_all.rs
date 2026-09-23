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

#[test]
fn moxon_behaves_like_a_moxon() {
    let lam = C / 162.0;
    let fmt = |mm: f64| format!("{mm}");
    let comp = antenna_designs::by_id("moxon").run(
        lam,
        2.0,
        &fmt,
        &HashMap::new(),
        &default_controls(),
        1.0,
    );
    let r = Prepared::new(&comp.output.solve, lam, 2.0, segment_cap()).solve(lam, true);
    let fb = r.gain_dbi([0.0, 0.0, 1.0]).unwrap() - r.gain_dbi([0.0, 0.0, -1.0]).unwrap();
    assert!((r.dbi.unwrap() - 6.1).abs() < 0.3, "{}", r.dbi.unwrap());
    assert!(fb > 20.0, "F/B {fb}");
    assert!((r.z - antenna_solver::C64::new(50.0, 0.0)).norm() < 12.0, "{}", r.z);
}

#[test]
fn moxon_pattern_metrics_look_like_a_moxon() {
    let lam = C / 162.0;
    let fmt = |mm: f64| format!("{mm}");
    let comp = antenna_designs::by_id("moxon").run(
        lam,
        2.0,
        &fmt,
        &HashMap::new(),
        &default_controls(),
        1.0,
    );
    let r = Prepared::new(&comp.output.solve, lam, 2.0, segment_cap()).solve(lam, true);
    let m = antenna_solver::analysis::analyse(&r, comp.output.scene.up()).unwrap();
    assert!(m.front_to_back > 20.0, "F/B {}", m.front_to_back);
    assert!(m.peak_dir[2] > 0.99, "peak {:?}", m.peak_dir);
    let az = m.beamwidth_azimuth.unwrap();
    let el = m.beamwidth_elevation.unwrap();
    assert!((50.0..90.0).contains(&az) && (80.0..160.0).contains(&el), "az {az} el {el}");
}
