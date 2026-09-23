use antenna_solver::geometry::Geometry;
use antenna_solver::nec::import;
use antenna_solver::solve::Prepared;
fn main() {
    for entry in std::fs::read_dir(std::env::args().nth(1).unwrap()).unwrap() {
        let p = entry.unwrap().path();
        let text = String::from_utf8_lossy(&std::fs::read(&p).unwrap()).into_owned();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        match import(&text) {
            Ok(r) => {
                let f = r.freq_mhz.unwrap_or(0.0);
                if f <= 0.0 {
                    println!("{name:<24} no FR");
                    continue;
                }
                let lam = 299_792.458 / f;
                let n: usize = r.geo.lines.iter().map(|l| l.pts.len() - 1).sum();
                if n > 1500 {
                    println!("{name:<24} {n} segs, skipped");
                    continue;
                }
                let res =
                    Prepared::new(&Geometry::Wire(r.geo.clone()), lam, 2.0, 5000).solve(lam, true);
                println!(
                    "{name:<24} {f:>8.2} MHz {n:>4} segs  Z {:>8.1}{:>+8.1}j  {:>6.2} dBi  warn {:?}",
                    res.z.re,
                    res.z.im,
                    res.dbi.unwrap_or(f64::NAN),
                    r.warnings
                );
            }
            Err(e) => println!("{name:<24} ERROR {e}"),
        }
    }
}
