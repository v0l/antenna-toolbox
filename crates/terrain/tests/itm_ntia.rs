use antenna_terrain::itm::{Climate, Params, point_to_point};

#[test]
fn matches_the_ntia_reference_cases() {
    let cases = include_str!("data/p2p.csv");
    let profiles: Vec<Vec<f64>> = include_str!("data/pfls.csv")
        .lines()
        .map(|l| {
            l.split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().parse().unwrap())
                .collect()
        })
        .collect();
    let mut n = 0;
    for (line, pfl) in cases.lines().skip(1).zip(&profiles) {
        let v: Vec<f64> = line.split(',').map(|s| s.trim().parse().unwrap()).collect();
        let np = pfl[0] as usize;
        let ground = &pfl[2..np + 3];
        let p = Params {
            climate: Climate::ALL[v[7] as usize - 1],
            n_0: v[4],
            vertical: v[6] == 1.0,
            epsilon: v[2],
            sigma: v[3],
            mdvar: v[11] as i32,
            time: v[8],
            location: v[9],
            situation: v[10],
        };
        let r = point_to_point(v[0], v[1], ground, pfl[1], v[5], &p).unwrap();
        assert!((r.loss_db - v[12]).abs() < 0.01, "case {n}: {} vs {}", r.loss_db, v[12]);
        n += 1;
    }
    assert_eq!(n, 5);
}

#[test]
fn matches_splat_on_its_own_profile() {
    let ground: Vec<f64> = include_str!("data/splat_galway_300deg.csv")
        .trim()
        .split(',')
        .map(|s| s.parse().unwrap())
        .collect();
    let r = point_to_point(10.0, 10.0, &ground, 60.255, 162.0, &Params::default()).unwrap();
    assert!(
        (r.loss_db - 164.80).abs() < 0.3,
        "SPLAT 1.4.2 -olditm gives 164.80, ours {}",
        r.loss_db
    );
}
