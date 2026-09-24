use antenna_designs::bands::{HF_BANDS, bands_from};
use antenna_designs::matching::{plan_match, swr_of_50};
use antenna_designs::{by_id, default_controls};
use antenna_solver::solve::{Prepared, sweep_cap};
use antenna_solver::units::C;
use rayon::prelude::*;
use std::collections::HashMap;

#[test]
fn random_wire_stays_tunable_on_every_band_above_its_lowest() {
    let fmt = |mm: f64| format!("{mm}");
    let failed: Vec<String> = HF_BANDS
        .par_iter()
        .filter_map(|low| {
            let lam0 = C / low.centre();
            let out = by_id("random")
                .run(lam0, 1.6, &fmt, &HashMap::new(), &default_controls(), 1.0)
                .output;
            let model = Prepared::new(&out.solve, lam0, 1.6, sweep_cap());
            let worst = bands_from(low.low)
                .iter()
                .flat_map(|b| b.probes())
                .map(|f| {
                    let z = model.solve(C / f, false).z;
                    swr_of_50(plan_match("random", z, C / f, 50.0).apply(z, f, f), 50.0)
                })
                .fold(1.0, f64::max);
            eprintln!("{:<6} worst SWR after the unun {worst:.1}", low.name);
            (worst > 8.0).then(|| format!("{} {worst:.1}", low.name))
        })
        .collect();
    assert!(failed.is_empty(), "{failed:?}");
}
