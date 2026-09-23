use antenna_vna::{detect, open};

#[test]
#[ignore = "needs a VNA on USB"]
fn sweeps_the_attached_vna() {
    let port = detect().into_iter().next().expect("no VNA found");
    let mut vna = open(&port).unwrap();
    eprintln!("{} on {}", vna.describe(), port.path);
    let pts = vna.sweep(600e6, 1100e6, 801, false).unwrap();
    assert_eq!(pts.len(), 801);
    assert!((pts[0].freq - 600e6).abs() < 1.0 && (pts[800].freq - 1100e6).abs() < 1e3);
    let best = pts.iter().min_by(|a, b| a.swr().total_cmp(&b.swr())).unwrap();
    eprintln!("min SWR {:.2} at {:.1} MHz, Z {:.1}", best.swr(), best.freq / 1e6, best.z(50.0));
    assert!(pts.iter().all(|p| p.s11.norm() < 1.5));
}
