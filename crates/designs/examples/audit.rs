use antenna_designs::{Build, DESIGNS, default_controls};
use antenna_solver::geometry::Geometry;
use antenna_solver::nec::export;
use antenna_solver::solve::{Prepared, segment_cap};
use antenna_solver::units::C;
use std::collections::HashMap;
fn main() {
    let f = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(162.0);
    let dia = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let lam = C / f;
    let fmt = |mm: f64| format!("{mm}");
    std::fs::create_dir_all("/tmp/audit").unwrap();
    for d in DESIGNS.iter().filter(|d| d.build != Build::Sheet) {
        let comp = d.run(lam, dia, &fmt, &HashMap::new(), &default_controls(), 1.0);
        let Geometry::Wire(geo) = &comp.output.solve else { continue };
        let r = Prepared::new(&comp.output.solve, lam, dia, segment_cap()).solve(lam, true);
        match export(geo, f, dia / 2.0, d.name) {
            Ok(deck) => std::fs::write(format!("/tmp/audit/{}.nec", d.id), deck).unwrap(),
            Err(e) => {
                println!("{} skip {e}", d.id);
                continue;
            }
        }
        println!("{} {:.3} {:.3} {:.3}", d.id, r.z.re, r.z.im, r.dbi.unwrap());
    }
}
