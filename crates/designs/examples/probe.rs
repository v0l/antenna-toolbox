use antenna_designs::{ControlId, by_id, default_controls};
use antenna_solver::solve::{Prepared, segment_cap, sweep_cap, swr_of, tune_to_resonance};
use antenna_solver::units::C;
use std::collections::HashMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = by_id(&a[1]);
    let f: f64 = a[2].parse().unwrap();
    let dia: f64 = a[3].parse().unwrap();
    let mut ov = HashMap::new();
    let mut ctl = default_controls();
    for kv in &a[4..] {
        let (k, v) = kv.split_once('=').unwrap();
        let v: f64 = v.parse().unwrap();
        if let Some(c) = ControlId::ALL.iter().find(|c| format!("{c:?}") == k) {
            ctl.insert(*c, v);
        } else {
            ov.insert(format!("{}:{}", d.id, k), v * C / f);
        }
    }
    let lam = C / f;
    let fmt = |mm: f64| format!("{mm}");
    let comp = d.run(lam, dia, &fmt, &ov, &ctl, 1.0);
    let r = Prepared::new(&comp.output.solve, lam, dia, segment_cap()).solve(lam, true);
    let b = comp.output.scene.beam_direction();
    let fb =
        r.gain_dbi(b).zip(r.gain_dbi([-b[0], -b[1], -b[2]])).map(|(x, y)| x - y).unwrap_or(0.0);
    let t = tune_to_resonance(
        |s| {
            Prepared::new(&d.run(lam, dia, &fmt, &ov, &ctl, s).output.solve, lam, dia, sweep_cap())
                .solve(lam, false)
                .z
        },
        |_, _| {},
    );
    let params: Vec<String> =
        comp.params.iter().map(|p| format!("{}={:.4}", p.name, p.val / lam)).collect();
    println!(
        "Z {:.1}{:+.1}j SWR {:.2} | {:.2} dBi fwd {:.2} F/B {:.1} AR {:.1} {} | tune {} | {}",
        r.z.re,
        r.z.im,
        swr_of(r.z, 50.0),
        r.dbi.unwrap(),
        r.gain_dbi(b).unwrap_or(0.0),
        fb,
        r.pol.map(|p| p.ar_db).unwrap_or(99.0),
        r.pol.map(|p| p.hand.label()).unwrap_or(""),
        t.map(|t| format!("x{:.3} Z {:.0}{:+.0}j", t.scale, t.z.re, t.z.im)).unwrap_or("-".into()),
        params.join(" ")
    );
}
