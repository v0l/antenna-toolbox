use antenna_solver::geometry::Geometry;
use antenna_solver::nec::import;
use antenna_solver::solve::Prepared;

fn table(out: &str, head: &str) -> Vec<([f64; 3], [f64; 3])> {
    let mut rows = Vec::new();
    for block in out.split(head).skip(1) {
        let mut started = false;
        for line in block.lines() {
            let v: Vec<f64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() != 9 {
                if started {
                    break;
                }
                continue;
            }
            started = true;
            rows.push(([v[0], v[1], v[2]], [v[3], v[5], v[7]]));
        }
    }
    rows
}

fn main() {
    let deck = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(&deck).unwrap();
    let out = std::fs::read_to_string(deck.replace(".nec", ".out")).unwrap();
    let r = import(&text).unwrap();
    let lam = 299_792.458 / r.freq_mhz.unwrap();
    let res = Prepared::new(&Geometry::Wire(r.geo.clone()), lam, 2.0, 5000).solve(lam, true);
    let near = res.near.unwrap();
    println!("Z {:.2}", res.z);
    let mut worst: f64 = 0.0;
    for (head, e) in [("NEAR ELECTRIC FIELDS", true), ("NEAR MAGNETIC FIELDS", false)] {
        for (p, nec) in table(&out, head) {
            let f = near.at([p[0] * 1000.0, p[1] * 1000.0, p[2] * 1000.0]);
            let v = if e { f.e } else { f.h };
            let ours = v.map(|c| c.norm());
            let tot_n = nec.iter().map(|x| x * x).sum::<f64>().sqrt();
            let tot_o = ours.iter().map(|x| x * x).sum::<f64>().sqrt();
            let err = (tot_o / tot_n - 1.0) * 100.0;
            worst = worst.max(err.abs());
            println!(
                "{} {:>6.2} {:>6.2} {:>6.2}  nec {:>10.4e}  ours {:>10.4e}  {:+.2}%",
                if e { "E" } else { "H" },
                p[0],
                p[1],
                p[2],
                tot_n,
                tot_o,
                err
            );
        }
    }
    println!("worst {worst:.2}%");
}
