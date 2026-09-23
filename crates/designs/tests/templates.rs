use antenna_designs::{DESIGNS, default_controls, dress};
use antenna_solver::geometry::{COPPER, WireProps};
use antenna_solver::solve::{Prepared, segment_cap, swr_of};
use antenna_solver::units::C;
use std::collections::HashMap;

const MATCHED: [&str; 10] = [
    "moxon",
    "yagi2",
    "yagi3",
    "biquad",
    "gp",
    "dipole",
    "vdip",
    "slimjim",
    "discone",
    "lindenblad",
];

#[test]
fn labels_and_matches_hold_for_every_template() {
    let lam = C / 162.0;
    let fmt = |mm: f64| format!("{mm}");
    let mut failed = Vec::new();
    for d in DESIGNS.iter() {
        let mut out = d.run(lam, 2.0, &fmt, &HashMap::new(), &default_controls(), 1.0).output;
        dress(&mut out.solve, WireProps { conductivity: Some(COPPER), insulation: None });
        let r = Prepared::new(&out.solve, lam, 2.0, segment_cap()).solve(lam, true);
        let label: f64 = d.gain.trim_end_matches(" dBi").parse().unwrap();
        let gain = r.dbi.unwrap();
        let swr = swr_of(r.z, 50.0);
        let ok = (gain - label).abs() <= 0.6 && (!MATCHED.contains(&d.id) || swr < 2.0);
        eprintln!(
            "{:<11} label {label:>4} solved {gain:>5.2} dBi  eff {:.2}  SWR {swr:>5.2} {}",
            d.id,
            r.efficiency.unwrap_or(1.0),
            if ok { "" } else { "FAIL" }
        );
        if !ok {
            failed.push(d.id);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
