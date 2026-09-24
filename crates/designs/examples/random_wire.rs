use antenna_designs::bands::{HF_BANDS, bands_from};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::solve::{Prepared, swr_of};
use antenna_solver::units::C;
use rayon::prelude::*;

fn worst(radiator: f64, counterpoise: f64, from: f64, wire: f64, cap: usize) -> (f64, Vec<f64>) {
    let g = 50.0;
    let geo = Geometry::Wire(WireGeometry::new(
        vec![vec![
            [-g - counterpoise, 0.0, 0.0],
            [-g, 0.0, 0.0],
            [g, 0.0, 0.0],
            [g + radiator, 0.0, 0.0],
        ]],
        [0.0; 3],
    ));
    let per: Vec<f64> = bands_from(from)
        .iter()
        .map(|b| {
            b.probes()
                .iter()
                .map(|&f| {
                    let lam = C / f;
                    let z = Prepared::new(&geo, lam, wire, cap).solve(lam, false).z;
                    swr_of(z / 9.0, 50.0)
                })
                .fold(1.0, f64::max)
        })
        .collect();
    (per.iter().copied().fold(1.0, f64::max), per)
}

fn main() {
    let wire = 1.6;
    for (i, low) in HF_BANDS.iter().enumerate() {
        let lam = C / low.centre();
        let cands: Vec<(f64, f64)> = (0..=100)
            .flat_map(|a| (0..=6).map(move |b| (0.2 + a as f64 * 0.005, 0.05 + b as f64 * 0.025)))
            .collect();
        let mut scored: Vec<(f64, f64, f64)> = cands
            .par_iter()
            .map(|&(l, c)| {
                let w = [0.97, 1.0, 1.03]
                    .iter()
                    .map(|s| worst(s * l * lam, c * lam, low.low, wire, 40).0)
                    .fold(1.0, f64::max);
                (w, l, c)
            })
            .collect();
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (_, l, c) = scored[0];
        let (w, per) = worst(l * lam, c * lam, low.low, wire, 300);
        let bands: Vec<String> = per.iter().map(|s| format!("{s:.1}")).collect();
        println!(
            "{i} {:<6} radiator {:.3} λ {:>6.2} m  counterpoise {:.3} λ {:>5.2} m  worst {w:.1}  [{}]",
            low.name,
            l,
            l * lam / 1000.0,
            c,
            c * lam / 1000.0,
            bands.join(" ")
        );
    }
}
