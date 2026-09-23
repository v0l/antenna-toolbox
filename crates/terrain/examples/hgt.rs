use antenna_terrain::Dem;
use std::io::Write;

fn main() {
    let args: Vec<i32> = std::env::args().skip(1).map(|a| a.parse().unwrap()).collect();
    let (lat0, lon0) = (args[0], args[1]);
    let dem = Dem::default();
    let name = format!(
        "{}{:02}{}{:03}.hgt",
        if lat0 >= 0 { 'N' } else { 'S' },
        lat0.abs(),
        if lon0 >= 0 { 'E' } else { 'W' },
        lon0.abs()
    );
    let mut out = Vec::with_capacity(1201 * 1201 * 2);
    for r in 0..1201 {
        for c in 0..1201 {
            let lat = f64::from(lat0 + 1) - r as f64 / 1200.0;
            let lon = f64::from(lon0) + c as f64 / 1200.0;
            let h = dem
                .elevation(lat.min(f64::from(lat0 + 1) - 1e-9), lon.min(f64::from(lon0 + 1) - 1e-9))
                .unwrap();
            out.extend_from_slice(&(h.round() as i16).to_be_bytes());
        }
    }
    std::fs::File::create(&name).unwrap().write_all(&out).unwrap();
    println!("{name}");
}
