use antenna_designs::{by_id, default_controls};
use antenna_solver::fdtd::{Options, gpu, run, run_gpu};
use antenna_solver::geometry::Geometry;
use antenna_solver::solve::volume_result;
use antenna_solver::units::C;
use std::collections::HashMap;

fn solve(id: &str, mhz: f64) -> (f64, f64, f64) {
    let fmt = |mm: f64| format!("{mm}");
    let out = by_id(id).run(C / mhz, 1.0, &fmt, &HashMap::new(), &default_controls(), 1.0).output;
    let Geometry::Volume(mut model) = out.solve else {
        panic!("{id} is not a volume model");
    };
    model.span = std::env::var("SPAN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.15);
    let opts = Options::default();
    let res = match gpu::block_on(gpu::adapter()) {
        Some(_) => gpu::block_on(run_gpu(&model, &opts, |_, _| true)).unwrap(),
        None => run(&model, &opts, |_, _| true).unwrap(),
    };
    let best = res
        .sweep
        .freqs
        .iter()
        .zip(&res.sweep.z)
        .min_by(|a, b| {
            ((a.1 - 50.0) / (a.1 + 50.0)).norm().total_cmp(&((b.1 - 50.0) / (b.1 + 50.0)).norm())
        })
        .unwrap();
    let swr = {
        let g = ((best.1 - 50.0) / (best.1 + 50.0)).norm();
        (1.0 + g) / (1.0 - g)
    };
    let r = volume_result(&res);
    eprintln!(
        "{id} at {mhz} MHz: best SWR {swr:.2} at {:.1} MHz, {:.2} dBi, efficiency {:.2}",
        best.0 / 1e6,
        r.dbi.unwrap_or(f64::NAN),
        r.efficiency.unwrap_or(f64::NAN)
    );
    (best.0 / 1e6, swr, r.dbi.unwrap_or(f64::NAN))
}

#[test]
fn the_patch_lands_near_its_frequency_with_patch_gain() {
    let (f, swr, dbi) = solve("patch", 2450.0);
    assert!((f / 2450.0 - 1.0).abs() < 0.05, "{f}");
    assert!(swr < 2.5, "{swr}");
    assert!((2.0..7.5).contains(&dbi), "{dbi}");
}

#[test]
fn the_inverted_f_lands_near_its_frequency() {
    let (f, swr, dbi) = solve("ifa", 868.0);
    assert!((f / 868.0 - 1.0).abs() < 0.08, "{f}");
    assert!(swr < 4.0, "{swr}");
    assert!((-4.0..4.0).contains(&dbi), "{dbi}");
}
