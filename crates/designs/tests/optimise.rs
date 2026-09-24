use antenna_designs::optimise::{Goal, Problem};
use antenna_designs::{by_id, default_controls};
use antenna_solver::geometry::{COPPER, WireProps};
use antenna_solver::units::C;
use std::collections::HashMap;

fn problem<'a>(
    id: &str,
    controls: &'a antenna_designs::Controls,
    base: &'a HashMap<String, f64>,
    goal: Goal,
    forward: [f64; 3],
) -> Problem<'a> {
    let d = by_id(id);
    let lam = C / 162.0;
    let fmt = |mm: f64| format!("{mm}");
    let params = d.run(lam, 2.0, &fmt, base, controls, 1.0).params;
    Problem {
        design: d,
        lam,
        wire: 2.0,
        props: WireProps { conductivity: Some(COPPER), insulation: None },
        controls,
        base,
        keys: params.iter().map(|p| (p.key.clone(), p.val)).collect(),
        goal,
        forward,
    }
}

#[test]
fn optimising_a_detuned_dipole_brings_it_back_to_resonance() {
    let controls = default_controls();
    let fmt = |mm: f64| format!("{mm}");
    let d = by_id("dipole");
    let lam = C / 162.0;
    let params = d.run(lam, 2.0, &fmt, &HashMap::new(), &controls, 1.0).params;
    let mut base = HashMap::new();
    for p in &params {
        base.insert(p.key.clone(), p.val * 1.12);
    }
    let goal = Goal { gain_weight: 0.0, band: 0.0, ..Goal::default() };
    let p = problem("dipole", &controls, &base, goal, [1.0, 0.0, 0.0]);
    let before = p.score(&vec![1.0; p.keys.len()]).unwrap();
    let (_, after) = p.run(200, |_, _, _| true);
    eprintln!("dipole SWR {:.2} -> {:.2}", before.worst_swr, after.worst_swr);
    assert!(before.worst_swr > 2.0);
    assert!(after.worst_swr < 1.5);
}

#[test]
fn optimising_a_yagi_does_not_lose_ground() {
    let controls = default_controls();
    let base = HashMap::new();
    let goal = Goal { fb_weight: 1.0, ..Goal::default() };
    let p = problem("yagi", &controls, &base, goal, [0.0, 0.0, 1.0]);
    let before = p.score(&vec![1.0; p.keys.len()]).unwrap();
    let (_, after) = p.run(120, |_, _, _| true);
    eprintln!(
        "yagi {} params: cost {:.2} -> {:.2}, SWR {:.2} -> {:.2}, gain {:.2} -> {:.2}, F/B {:.1} -> {:.1}",
        p.keys.len(),
        before.cost,
        after.cost,
        before.worst_swr,
        after.worst_swr,
        before.gain,
        after.gain,
        before.fb,
        after.fb
    );
    assert!(after.cost <= before.cost);
}
