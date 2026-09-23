use antenna_terrain::p528::{Mode, p528};

#[test]
fn matches_the_ntia_readme_cases() {
    for (d, h1, h2, f, v, p, want) in [
        (15.0, 10.0, 1000.0, 500.0, false, 50.0, 110.0),
        (100.0, 100.0, 15_000.0, 3600.0, false, 90.0, 151.6),
        (1500.0, 15.0, 10_000.0, 5700.0, false, 10.0, 293.4),
        (30.0, 8.0, 20_000.0, 22_000.0, true, 50.0, 151.1),
    ] {
        let r = p528(d, h1, h2, f, v, p).unwrap();
        assert!((r.loss_db - want).abs() < 0.05, "{d} km: {} vs {want}", r.loss_db);
    }
}

#[test]
fn matches_the_ntia_library_on_four_hundred_paths() {
    let mut worst = 0.0f64;
    for line in include_str!("data/p528.csv").lines() {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let r = p528(v[0], v[1], v[2], v[3], v[4] == 1.0, v[5]).unwrap();
        let mode = match r.mode {
            Mode::LineOfSight => 1.0,
            Mode::Diffraction => 2.0,
            Mode::Troposcatter => 3.0,
        };
        assert_eq!(mode, v[7], "{line}");
        worst = worst.max((r.loss_db - v[6]).abs());
        assert!((r.loss_db - v[6]).abs() < 0.01, "{line}: {}", r.loss_db);
    }
    assert!(worst < 0.01);
}
