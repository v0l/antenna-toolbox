use crate::App;
use std::collections::BTreeMap;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn snapshot(app: &App) -> BTreeMap<&'static str, String> {
    let (d, p, v) = (&app.design, &app.path, &app.vna);
    let mut m = BTreeMap::new();
    m.insert("design", d.design.id.to_string());
    m.insert("freq", d.freq.to_string());
    m.insert("wire", d.wire.to_string());
    m.insert("metal", format!("{:?}", d.material));
    m.insert("span", d.span.to_string());
    m.insert("z0", d.z0.to_string());
    m.insert("site_lat", p.site.0.to_string());
    m.insert("site_lon", p.site.1.to_string());
    m.insert("site_agl", p.site_agl.to_string());
    m.insert("target_lat", p.target.0.to_string());
    m.insert("target_lon", p.target.1.to_string());
    m.insert("target_agl", p.target_agl.to_string());
    m.insert("target_asl", p.target_asl.to_string());
    m.insert("site_transmits", p.site_transmits.to_string());
    m.insert("k", p.k.to_string());
    m.insert("path_freq", p.freq.to_string());
    m.insert("path_gain", p.gain_dbi.to_string());
    m.insert("tx_dbm", p.tx_dbm.to_string());
    m.insert("far_dbi", p.far_dbi.to_string());
    m.insert("cable_db", p.cable_db.to_string());
    m.insert("sens_dbm", p.sens_dbm.to_string());
    m.insert("itm_climate", (p.itm.climate as u8).to_string());
    m.insert("itm_ground", p.ground_index().to_string());
    m.insert("itm_vertical", p.itm.vertical.to_string());
    m.insert("itm_sea_auto", p.itm.sea_auto.to_string());
    m.insert("itm_n0", p.itm.n_0.to_string());
    m.insert("itm_time", p.itm.time.to_string());
    m.insert("itm_situation", p.itm.situation.to_string());
    m.insert("radius_km", p.radius_km.to_string());
    m.insert("site_clutter", p.site_clutter.map_or(0, |c| c as u8).to_string());
    m.insert("far_clutter", p.far_clutter.map_or(0, |c| c as u8).to_string());
    m.insert("street_m", p.street_m.to_string());
    m.insert("delta_n", p.delta_n.to_string());
    for (tag, slot) in [("", &p.site_pattern), ("far_", &p.far_pattern)] {
        m.insert(leak(format!("{tag}heading")), slot.mount.heading.to_string());
        m.insert(leak(format!("{tag}tilt")), slot.mount.tilt.to_string());
        m.insert(leak(format!("{tag}roll")), slot.mount.roll.to_string());
        m.insert(leak(format!("{tag}aim")), slot.aim.to_string());
    }
    m.insert("vna_points", v.points.to_string());
    m.insert("vna_target", v.target.to_string());
    m.insert("vna_z0", v.z0.to_string());
    m.insert("vna_cable", v.cable.to_string());
    m.insert("vna_cable_m", v.cable_m.to_string());
    m.insert(
        "vna_deembed",
        match v.deembed {
            crate::vna::Deembed::Off => "off",
            crate::vna::Deembed::Cable => "cable",
            crate::vna::Deembed::Measured => "measured",
        }
        .into(),
    );
    if let Some(f) = v.open_fit {
        m.insert("vna_open_fit", format!("{},{},{}", f.delay_s, f.k_sqrt, f.k_lin));
    }
    m
}

pub fn save(app: &App) {
    let text: String = snapshot(app).iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    crate::store::write("state.txt", &text);
}

pub fn load(app: &mut App) {
    let Some(text) = crate::store::read("state.txt") else {
        return;
    };
    let m: BTreeMap<&str, &str> = text.lines().filter_map(|l| l.split_once('=')).collect();
    let num = |k: &str, into: &mut f64| {
        if let Some(v) = m.get(k).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite()) {
            *into = v;
        }
    };
    if let Some(id) = m.get("design") {
        app.design.design = antenna_designs::by_id(id);
    }
    num("freq", &mut app.design.freq);
    num("wire", &mut app.design.wire);
    if let Some(found) = m
        .get("metal")
        .and_then(|m| crate::design::Material::ALL.iter().find(|x| format!("{x:?}") == *m))
    {
        app.design.material = *found;
    }
    num("span", &mut app.design.span);
    num("z0", &mut app.design.z0);
    let p = &mut app.path;
    num("site_lat", &mut p.site.0);
    num("site_lon", &mut p.site.1);
    num("site_agl", &mut p.site_agl);
    num("target_lat", &mut p.target.0);
    num("target_lon", &mut p.target.1);
    num("target_agl", &mut p.target_agl);
    if let Some(v) = m.get("target_asl").and_then(|v| v.parse().ok()) {
        p.target_asl = v;
    }
    if let Some(v) = m.get("site_transmits").and_then(|v| v.parse().ok()) {
        p.site_transmits = v;
    }
    num("k", &mut p.k);
    num("path_freq", &mut p.freq);
    num("path_gain", &mut p.gain_dbi);
    num("tx_dbm", &mut p.tx_dbm);
    num("far_dbi", &mut p.far_dbi);
    num("cable_db", &mut p.cable_db);
    num("sens_dbm", &mut p.sens_dbm);
    num("itm_n0", &mut p.itm.n_0);
    num("itm_time", &mut p.itm.time);
    num("itm_situation", &mut p.itm.situation);
    num("radius_km", &mut p.radius_km);
    num("street_m", &mut p.street_m);
    num("delta_n", &mut p.delta_n);
    let clutter = |k: &str| {
        m.get(k)
            .and_then(|v| v.parse::<u8>().ok())
            .and_then(antenna_terrain::p2108::Clutter::from_code)
    };
    p.site_clutter = clutter("site_clutter");
    p.far_clutter = clutter("far_clutter");
    for (tag, slot) in [("", &mut p.site_pattern), ("far_", &mut p.far_pattern)] {
        num(&format!("{tag}heading"), &mut slot.mount.heading);
        num(&format!("{tag}tilt"), &mut slot.mount.tilt);
        num(&format!("{tag}roll"), &mut slot.mount.roll);
        if let Some(v) = m.get(format!("{tag}aim").as_str()).and_then(|v| v.parse().ok()) {
            slot.aim = v;
        }
        slot.restore();
    }
    if let Some(c) = m
        .get("itm_climate")
        .and_then(|v| v.parse::<usize>().ok())
        .and_then(|c| antenna_terrain::itm::Climate::ALL.get(c.wrapping_sub(1)))
    {
        p.itm.climate = *c;
    }
    if let Some(g) = m
        .get("itm_ground")
        .and_then(|v| v.parse::<usize>().ok())
        .and_then(|g| crate::path::GROUNDS.get(g))
    {
        p.itm.epsilon = g.1;
        p.itm.sigma = g.2;
    }
    if let Some(v) = m.get("itm_vertical").and_then(|v| v.parse().ok()) {
        p.itm.vertical = v;
    }
    if let Some(v) = m.get("itm_sea_auto").and_then(|v| v.parse().ok()) {
        p.itm.sea_auto = v;
    }
    num("vna_target", &mut app.vna.target);
    num("vna_z0", &mut app.vna.z0);
    if let Some(n) = m.get("vna_points").and_then(|v| v.parse().ok()) {
        app.vna.points = n;
    }
    num("vna_cable_m", &mut app.vna.cable_m);
    if let Some(n) = m.get("vna_cable").and_then(|v| v.parse::<usize>().ok()) {
        app.vna.cable = n.min(antenna_rf::cable::CABLES.len() - 1);
    }
    if let Some(v) = m.get("vna_open_fit") {
        let f: Vec<f64> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        if let [delay_s, k_sqrt, k_lin] = f[..] {
            app.vna.open_fit = Some(antenna_rf::cable::OpenFit { delay_s, k_sqrt, k_lin });
        }
    }
    app.vna.deembed = match m.get("vna_deembed").copied() {
        Some("cable") => crate::vna::Deembed::Cable,
        Some("measured") if app.vna.open_fit.is_some() => crate::vna::Deembed::Measured,
        _ => crate::vna::Deembed::Off,
    };
}
