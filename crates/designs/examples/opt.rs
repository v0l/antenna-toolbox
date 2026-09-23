use antenna_designs::{by_id, default_controls};
use antenna_solver::solve::{Prepared, sweep_cap, swr_of};
use antenna_solver::units::C;
use rayon::prelude::*;
use std::collections::HashMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = by_id(&a[1]);
    let f: f64 = a[2].parse().unwrap();
    let dia: f64 = a[3].parse().unwrap();
    let lam = C / f;
    let axes: Vec<(String, Vec<f64>)> = a[4..]
        .iter()
        .map(|s| {
            let (k, r) = s.split_once('=').unwrap();
            let v: Vec<f64> = r.split(':').map(|x| x.parse().unwrap()).collect();
            let n = ((v[1] - v[0]) / v[2]).round() as usize;
            (k.to_string(), (0..=n).map(|i| v[0] + v[2] * i as f64).collect())
        })
        .collect();
    let mut combos: Vec<Vec<f64>> = vec![vec![]];
    for (_, vals) in &axes {
        combos = combos
            .into_iter()
            .flat_map(|c| {
                vals.iter().map(move |v| {
                    let mut c = c.clone();
                    c.push(*v);
                    c
                })
            })
            .collect();
    }
    let mut res: Vec<(f64, Vec<f64>, String)> = combos
        .par_iter()
        .map(|c| {
            let mut ov = HashMap::new();
            for ((k, _), v) in axes.iter().zip(c) {
                ov.insert(format!("{}:{}", d.id, k), v * lam);
            }
            let fmt = |mm: f64| format!("{mm}");
            let comp = d.run(lam, dia, &fmt, &ov, &default_controls(), 1.0);
            let z = Prepared::new(&comp.output.solve, lam, dia, sweep_cap()).solve(lam, false).z;
            (swr_of(z, 50.0), c.clone(), format!("{:.1}{:+.1}j", z.re, z.im))
        })
        .collect();
    res.sort_by(|x, y| x.0.total_cmp(&y.0));
    for r in res.iter().take(5) {
        println!(
            "SWR {:.3} Z {} {:?}",
            r.0,
            r.2,
            r.1.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>()
        );
    }
}
