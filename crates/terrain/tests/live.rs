use antenna_terrain::Dem;

#[test]
#[ignore = "downloads a Copernicus tile"]
fn croagh_patrick_summit_is_where_it_should_be() {
    let dem = Dem::default();
    let summit = (0..40)
        .flat_map(|i| (0..40).map(move |j| (53.755 + i as f64 * 0.0003, -9.665 + j as f64 * 0.0003)))
        .map(|(lat, lon)| dem.elevation(lat, lon).unwrap())
        .fold(f64::MIN, f64::max);
    eprintln!("summit {summit:.1} m");
    assert!((summit - 764.0).abs() < 15.0, "{summit}");
    let sea = dem.elevation(53.80, -9.90).unwrap();
    assert!(sea.abs() < 3.0, "sea {sea}");
}
