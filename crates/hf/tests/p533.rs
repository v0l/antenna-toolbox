use antenna_hf::p372::{Coefficients, ManMade};
use antenna_hf::p533::{Context, Deciles, Input, IonMaps, Location, Pattern, run};
use std::collections::HashMap;
use std::path::PathBuf;

const D2R: f64 = 0.0174532925;

fn maps(month: usize) -> IonMaps {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/p533-data");
    std::fs::create_dir_all(&dir).unwrap();
    let name = antenna_hf::ionos_file(month);
    let path = dir.join(&name);
    if !path.exists() {
        let url = format!("{}/{name}", antenna_hf::IONOS_URL);
        let ok = std::process::Command::new("curl")
            .args(["-sfL", "-o", path.to_str().unwrap(), &url])
            .status()
            .unwrap()
            .success();
        assert!(ok, "could not fetch {url}");
    }
    IonMaps::from_bin(&std::fs::read(path).unwrap()).unwrap()
}

fn grid(lat: f64, lng: f64) -> Location {
    let i = ((lat + 55.0) / 20.0).round();
    let j = ((lng + 175.0) / 35.0).round();
    Location { lat: -55.0 * D2R + i * (20.0 * D2R), lng: -175.0 * D2R + j * (35.0 * D2R) }
}

fn noise(s: &str) -> ManMade {
    match s {
        "RURAL" => ManMade::Rural,
        "RESIDENTIAL" => ManMade::Residential,
        "QUIETRURAL" => ManMade::QuietRural,
        "CITY" => ManMade::City,
        "NOISY" => ManMade::Noisy,
        _ => ManMade::Quiet,
    }
}

#[test]
#[ignore = "downloads four monthly ionospheric maps from the ITU repository"]
fn matches_iturhfprop() {
    let text = include_str!("data/p533.csv");
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap();
    let deciles = Deciles::default();
    let iso = Pattern::isotropic(0.0);
    let mut cache: HashMap<usize, (IonMaps, Coefficients)> = HashMap::new();
    let mut worst: HashMap<&str, f64> = HashMap::new();
    let mut n = 0;
    let mut bad: HashMap<&str, usize> = HashMap::new();
    for line in lines {
        let r: Vec<&str> = line.split(',').collect();
        let v = |name: &str| r[col(name)].trim().parse::<f64>().unwrap();
        let month = v("month") as usize - 1;
        let (m, c) =
            cache.entry(month).or_insert_with(|| (maps(month), Coefficients::month(month)));
        let ctx = Context { maps: m, deciles: &deciles, noise: c, tx_ant: &iso, rx_ant: &iso };
        let input = Input {
            month,
            hour: v("hour") as usize - 1,
            ssn: v("ssn") as i32,
            frequency: v("freq"),
            bw: 3000.0,
            txpower: 0.0,
            snrr: 10.0,
            snrxxp: 90,
            tx: Location { lat: v("txlat") * D2R, lng: v("txlng") * D2R },
            rx: grid(v("rxlat"), v("rxlng")),
            long_path: false,
            man_made: noise(r[col("noise")]),
        };
        let o = run(&input, &ctx);
        let checks = [
            ("d", o.distance, v("d")),
            ("bmuf", o.bmuf, v("bmuf")),
            ("muf90", o.muf90, v("muf90")),
            ("muf10", o.muf10, v("muf10")),
            ("opmuf", o.opmuf, v("opmuf")),
            ("e", o.ep, v("e")),
            ("pr", o.pr, v("pr")),
            ("faa", o.noise.fa_a, v("faa")),
            ("fam", o.noise.fa_m, v("fam")),
            ("famt", o.noise.fam_t, v("famt")),
            ("snr", o.snr, v("snr")),
            ("snrxx", o.snrxx, v("snrxx")),
            ("bcr", o.bcr, v("bcr")),
        ];
        for (name, ours, theirs) in checks {
            let err = (ours - theirs).abs();
            let w = worst.entry(name).or_insert(0.0);
            *w = w.max(err);
            if !(err <= 0.011 || (theirs.abs() > 1e5 && ours.abs() > 1e5)) {
                *bad.entry(name).or_insert(0) += 1;
                if bad[name] <= 3 {
                    eprintln!("{name}: ours {ours} vs {theirs} on {line}");
                }
            }
        }
        n += 1;
    }
    eprintln!("{n} cases, worst {worst:?}");
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn noise_matches_iturhfprop() {
    let text = include_str!("data/p533.csv");
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap();
    let mut n = 0;
    for line in lines {
        let r: Vec<&str> = line.split(',').collect();
        let v = |name: &str| r[col(name)].trim().parse::<f64>().unwrap();
        let month = v("month") as usize - 1;
        let c = Coefficients::month(month);
        let rx = grid(v("rxlat"), v("rxlng"));
        let o = antenna_hf::p372::noise(
            &c,
            noise(r[col("noise")]),
            v("hour") as i32 - 1,
            rx.lng,
            rx.lat,
            v("freq"),
        );
        for (name, ours) in [
            ("faa", o.fa_a),
            ("fam", o.fa_m),
            ("fag", o.fa_g),
            ("dua", o.du_a),
            ("dla", o.dl_a),
            ("famt", o.fam_t),
        ] {
            assert!((ours - v(name)).abs() <= 0.006, "{name}: {ours} vs {} on {line}", v(name));
        }
        n += 1;
    }
    assert!(n > 1000);
}
