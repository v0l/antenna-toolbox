use crate::App;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("antenna-toolbox").join("state.txt"))
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
    m.insert("itm_n0", p.itm.n_0.to_string());
    m.insert("itm_time", p.itm.time.to_string());
    m.insert("itm_situation", p.itm.situation.to_string());
    m.insert("radius_km", p.radius_km.to_string());
    m.insert("vna_points", v.points.to_string());
    m.insert("vna_target", v.target.to_string());
    m.insert("vna_z0", v.z0.to_string());
    m
}

pub fn save(app: &App) {
    let Some(path) = file() else {
        return;
    };
    let text: String = snapshot(app).iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, text);
}

pub fn load(app: &mut App) {
    let Some(text) = file().and_then(|p| std::fs::read_to_string(p).ok()) else {
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
    if let Some(m) = m.get("metal") {
        if let Some(found) = crate::design::Material::ALL.iter().find(|x| format!("{x:?}") == *m) {
            app.design.material = *found;
        }
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
    num("vna_target", &mut app.vna.target);
    num("vna_z0", &mut app.vna.z0);
    if let Some(n) = m.get("vna_points").and_then(|v| v.parse().ok()) {
        app.vna.points = n;
    }
}
