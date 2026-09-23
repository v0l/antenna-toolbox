use crate::dem::Dem;
use crate::itm::{self, Params};

pub const EARTH_RADIUS: f64 = 6_371_008.8;
pub const C: f64 = 299_792_458.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    pub fn new(lat: f64, lon: f64) -> Self {
        LatLon { lat, lon }
    }

    pub fn distance_to(self, b: LatLon) -> f64 {
        let (p1, p2) = (self.lat.to_radians(), b.lat.to_radians());
        let dp = p2 - p1;
        let dl = (b.lon - self.lon).to_radians();
        let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
        2.0 * EARTH_RADIUS * h.sqrt().asin()
    }

    pub fn bearing_to(self, b: LatLon) -> f64 {
        let (p1, p2) = (self.lat.to_radians(), b.lat.to_radians());
        let dl = (b.lon - self.lon).to_radians();
        let y = dl.sin() * p2.cos();
        let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
        (y.atan2(x).to_degrees() + 360.0) % 360.0
    }

    pub fn destination(self, bearing_deg: f64, dist: f64) -> LatLon {
        let d = dist / EARTH_RADIUS;
        let th = bearing_deg.to_radians();
        let p1 = self.lat.to_radians();
        let l1 = self.lon.to_radians();
        let p2 = (p1.sin() * d.cos() + p1.cos() * d.sin() * th.cos()).asin();
        let l2 = l1 + (th.sin() * d.sin() * p1.cos()).atan2(d.cos() - p1.sin() * p2.sin());
        LatLon { lat: p2.to_degrees(), lon: ((l2.to_degrees() + 540.0) % 360.0) - 180.0 }
    }

    pub fn interpolate(self, b: LatLon, f: f64) -> LatLon {
        let d = self.distance_to(b);
        if d == 0.0 {
            return self;
        }
        self.destination(self.bearing_to(b), d * f)
    }
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub at: LatLon,
    pub height_agl: f64,
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub dist: f64,
    pub ground: f64,
    pub bulge: f64,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub a: Endpoint,
    pub b: Endpoint,
    pub k: f64,
    pub samples: Vec<Sample>,
}

impl Profile {
    pub fn fetch(
        dem: &Dem,
        a: Endpoint,
        b: Endpoint,
        k: f64,
        step: f64,
    ) -> Result<Profile, String> {
        let d = a.at.distance_to(b.at);
        let n = ((d / step).ceil() as usize).clamp(2, 20_000);
        let samples = (0..=n)
            .map(|i| {
                let f = i as f64 / n as f64;
                let p = a.at.interpolate(b.at, f);
                let dist = d * f;
                Ok(Sample {
                    dist,
                    ground: dem.elevation(p.lat, p.lon)?.max(0.0),
                    bulge: dist * (d - dist) / (2.0 * k * EARTH_RADIUS),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Profile { a, b, k, samples })
    }

    pub fn from_ground(a: Endpoint, b: Endpoint, k: f64, ground: &[f64]) -> Profile {
        let d = a.at.distance_to(b.at);
        let n = ground.len() - 1;
        let samples = ground
            .iter()
            .enumerate()
            .map(|(i, &g)| {
                let dist = d * i as f64 / n as f64;
                Sample { dist, ground: g, bulge: dist * (d - dist) / (2.0 * k * EARTH_RADIUS) }
            })
            .collect();
        Profile { a, b, k, samples }
    }

    pub fn length(&self) -> f64 {
        self.samples.last().map(|s| s.dist).unwrap_or(0.0)
    }

    pub fn antenna_a(&self) -> f64 {
        self.samples[0].ground + self.a.height_agl
    }

    pub fn antenna_b(&self) -> f64 {
        self.samples.last().map(|s| s.ground).unwrap_or(0.0) + self.b.height_agl
    }

    pub fn los_at(&self, dist: f64) -> f64 {
        let d = self.length();
        self.antenna_a() + (self.antenna_b() - self.antenna_a()) * dist / d
    }
}

#[derive(Clone, Debug)]
pub struct Obstacle {
    pub dist: f64,
    pub height: f64,
    pub v: f64,
}

#[derive(Clone, Debug)]
pub struct Analysis {
    pub distance: f64,
    pub bearing: f64,
    pub freq_mhz: f64,
    pub fspl_db: f64,
    pub diffraction_db: f64,
    pub itm: Result<itm::Result, itm::Error>,
    pub loss_db: f64,
    pub airborne: bool,
    pub worst_clearance: f64,
    pub worst_at: f64,
    pub line_of_sight: bool,
    pub fresnel_clear: bool,
    pub takeoff_deg: f64,
    pub horizon_deg: f64,
    pub arrival_deg: f64,
    pub obstacles: Vec<Obstacle>,
}

pub fn fresnel_radius(lam: f64, d1: f64, d2: f64) -> f64 {
    if d1 <= 0.0 || d2 <= 0.0 { 0.0 } else { (lam * d1 * d2 / (d1 + d2)).sqrt() }
}

pub fn knife_edge_db(v: f64) -> f64 {
    if v <= -0.78 { 0.0 } else { 6.9 + 20.0 * (((v - 0.1).powi(2) + 1.0).sqrt() + v - 0.1).log10() }
}

fn g_height(y: f64) -> f64 {
    if y > 2.0 {
        17.6 * (y - 1.1).sqrt() - 5.0 * (y - 1.1).log10() - 8.0
    } else {
        20.0 * (y + 0.1 * y.powi(3)).max(1e-9).log10()
    }
}

pub fn smooth_earth_db(freq_mhz: f64, dist: f64, h1: f64, h2: f64, k: f64) -> f64 {
    let ae = k * EARTH_RADIUS / 1000.0;
    let d_km = dist / 1000.0;
    let horizon = (radio_horizon(h1, k) + radio_horizon(h2, k)) / 1000.0;
    if d_km <= horizon {
        return 0.0;
    }
    let x = 2.188 * freq_mhz.cbrt() * ae.powf(-2.0 / 3.0) * d_km;
    let f = if x >= 1.6 {
        11.0 + 10.0 * x.log10() - 17.6 * x
    } else {
        -20.0 * x.log10() - 5.6488 * x.powf(1.425)
    };
    let y = |h: f64| 9.575e-3 * freq_mhz.powf(2.0 / 3.0) * ae.powf(-1.0 / 3.0) * h.max(0.0);
    (-(f + g_height(y(h1)) + g_height(y(h2)))).max(0.0)
}

pub fn radio_horizon(height: f64, k: f64) -> f64 {
    (2.0 * k * EARTH_RADIUS * height.max(0.0)).sqrt()
}

fn worst_edge(
    p: &Profile,
    lam: f64,
    i0: usize,
    i1: usize,
    h0: f64,
    h1: f64,
) -> Option<(usize, f64)> {
    let (d0, d1) = (p.samples[i0].dist, p.samples[i1].dist);
    let span = d1 - d0;
    if span <= 0.0 {
        return None;
    }
    let top = |i: usize| p.samples[i].ground + p.samples[i].bulge;
    (i0 + 1..i1)
        .filter(|&i| top(i) >= top(i - 1) && top(i) >= top(i + 1))
        .map(|i| {
            let s = &p.samples[i];
            let (a, b) = (s.dist - d0, d1 - s.dist);
            let los = h0 + (h1 - h0) * a / span;
            let h = s.ground + s.bulge - los;
            (i, h * (2.0 * span / (lam * a * b)).sqrt())
        })
        .max_by(|x, y| x.1.total_cmp(&y.1))
}

fn deygout(
    p: &Profile,
    lam: f64,
    i0: usize,
    i1: usize,
    h0: f64,
    h1: f64,
    depth: u8,
    out: &mut Vec<Obstacle>,
) -> f64 {
    let Some((im, v)) = worst_edge(p, lam, i0, i1, h0, h1) else {
        return 0.0;
    };
    if v <= -0.78 {
        return 0.0;
    }
    let s = &p.samples[im];
    let top = s.ground + s.bulge;
    out.push(Obstacle { dist: s.dist, height: top, v });
    let mut loss = knife_edge_db(v);
    if depth > 0 {
        loss += deygout(p, lam, i0, im, h0, top, depth - 1, out);
        loss += deygout(p, lam, im, i1, top, h1, depth - 1, out);
    }
    loss
}

pub fn analyse(p: &Profile, freq_mhz: f64, params: &Params) -> Analysis {
    let lam = C / (freq_mhz * 1e6);
    let d = p.length();
    let (ha, hb) = (p.antenna_a(), p.antenna_b());
    let mut worst = f64::INFINITY;
    let mut worst_at = 0.0;
    let mut los = true;
    let mut fresnel = true;
    let mut horizon = f64::NEG_INFINITY;
    let curve = 2.0 * p.k * EARTH_RADIUS;
    for s in &p.samples[1..p.samples.len() - 1] {
        let top = s.ground + s.bulge;
        let clearance = p.los_at(s.dist) - top;
        let r1 = fresnel_radius(lam, s.dist, d - s.dist);
        if clearance < 0.0 {
            los = false;
        }
        if clearance < 0.6 * r1 {
            fresnel = false;
        }
        if clearance - 0.6 * r1 < worst {
            worst = clearance - 0.6 * r1;
            worst_at = s.dist;
        }
        let elev = ((s.ground - ha) / s.dist - s.dist / curve).atan().to_degrees();
        horizon = horizon.max(elev);
    }
    let takeoff = ((hb - ha) / d - d / curve).atan().to_degrees();
    let arrival = ((ha - hb) / d - d / curve).atan().to_degrees();
    let mut obstacles = Vec::new();
    let floor = p.samples.iter().map(|s| s.ground).fold(f64::INFINITY, f64::min);
    let diffraction = deygout(p, lam, 0, p.samples.len() - 1, ha, hb, 1, &mut obstacles)
        .max(smooth_earth_db(freq_mhz, d, ha - floor, hb - floor, p.k));
    obstacles.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    let ground: Vec<f64> = p.samples.iter().map(|s| s.ground).collect();
    let step = d / (ground.len() - 1) as f64;
    let itm = itm::point_to_point(
        p.a.height_agl.max(0.5),
        p.b.height_agl.max(0.5),
        &ground,
        step,
        freq_mhz,
        params,
    );
    let fspl = 20.0 * (4.0 * std::f64::consts::PI * d / lam).log10();
    Analysis {
        loss_db: itm.as_ref().map(|r| r.loss_db).unwrap_or(fspl + diffraction),
        airborne: matches!(itm, Err(itm::Error::TerminalHeight)),
        itm,
        distance: d,
        bearing: p.a.at.bearing_to(p.b.at),
        freq_mhz,
        fspl_db: fspl,
        diffraction_db: diffraction,
        worst_clearance: worst,
        worst_at,
        line_of_sight: los,
        fresnel_clear: fresnel,
        takeoff_deg: takeoff.max(if los { f64::NEG_INFINITY } else { horizon }),
        horizon_deg: horizon,
        arrival_deg: arrival,
        obstacles,
    }
}

pub const ITM_CEILING: f64 = 3000.0;

pub fn path_loss(
    ground: &[f64],
    step: f64,
    h_a: f64,
    h_b: f64,
    freq_mhz: f64,
    params: &Params,
) -> f64 {
    if h_a <= ITM_CEILING
        && h_b <= ITM_CEILING
        && let Ok(r) =
            itm::point_to_point(h_a.max(0.5), h_b.max(0.5), ground, step, freq_mhz, params)
    {
        return r.loss_db;
    }
    free_space_and_diffraction(ground, step, h_a, h_b, freq_mhz, 4.0 / 3.0)
}

pub fn free_space_and_diffraction(
    ground: &[f64],
    step: f64,
    h_a: f64,
    h_b: f64,
    freq_mhz: f64,
    k: f64,
) -> f64 {
    let d = step * (ground.len() - 1) as f64;
    let lam = C / (freq_mhz * 1e6);
    let fspl = 20.0 * (4.0 * std::f64::consts::PI * d.max(1.0) / lam).log10();
    let at = LatLon::new(0.0, 0.0);
    let p = Profile::from_ground(
        Endpoint { at, height_agl: h_a },
        Endpoint { at: at.destination(90.0, d), height_agl: h_b },
        k,
        ground,
    );
    let (ha, hb) = (p.antenna_a(), p.antenna_b());
    let floor = ground.iter().copied().fold(f64::INFINITY, f64::min);
    let mut edges = Vec::new();
    let knife = deygout(&p, lam, 0, ground.len() - 1, ha, hb, 1, &mut edges);
    fspl + knife.max(smooth_earth_db(freq_mhz, d, ha - floor, hb - floor, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(lat: f64, lon: f64, h: f64) -> Endpoint {
        Endpoint { at: LatLon::new(lat, lon), height_agl: h }
    }

    #[test]
    fn great_circle_matches_a_known_distance() {
        let dublin = LatLon::new(53.3498, -6.2603);
        let galway = LatLon::new(53.2707, -9.0568);
        let d = dublin.distance_to(galway);
        assert!((d - 186_500.0).abs() < 1500.0, "{d}");
        let b = dublin.bearing_to(galway);
        assert!((b - 267.0).abs() < 2.0, "{b}");
        let back = dublin.destination(b, d);
        assert!(back.distance_to(galway) < 1.0);
    }

    #[test]
    fn knife_edge_follows_itu_p526() {
        assert_eq!(knife_edge_db(-1.0), 0.0);
        assert!((knife_edge_db(0.0) - 6.0).abs() < 0.1);
        assert!((knife_edge_db(1.0) - 13.9).abs() < 0.2);
        assert!((knife_edge_db(2.4) - 20.5).abs() < 0.3);
    }

    #[test]
    fn horizon_matches_the_four_thirds_rule() {
        let d = radio_horizon(120.0, 4.0 / 3.0);
        assert!((d / 1000.0 - 4.12 * 120f64.sqrt()).abs() < 0.3, "{}", d / 1000.0);
    }

    #[test]
    fn flat_sea_path_past_the_horizon_is_obstructed_by_the_bulge() {
        let a = ep(53.0, -9.0, 10.0);
        let b = ep(53.0, -8.0, 10.0);
        let p = Profile::from_ground(a, b, 4.0 / 3.0, &vec![0.0; 401]);
        let r = analyse(&p, 162.0, &Params::default());
        assert!((r.distance - 66_900.0).abs() < 500.0);
        assert!(!r.line_of_sight);
        assert!((r.diffraction_db - 54.0).abs() < 3.0, "{}", r.diffraction_db);
        let near = Profile::from_ground(
            ep(53.0, -9.0, 30.0),
            ep(53.0, -8.8, 30.0),
            4.0 / 3.0,
            &vec![0.0; 401],
        );
        let rn = analyse(&near, 162.0, &Params::default());
        assert!(rn.line_of_sight && !rn.fresnel_clear && rn.diffraction_db < 6.0);
    }

    #[test]
    fn an_aircraft_above_the_itm_ceiling_falls_back_to_free_space_and_diffraction() {
        let sea = vec![0.0; 401];
        let near = path_loss(&sea, 100.0, 10.0, 10_000.0, 1090.0, &Params::default());
        let fspl = 20.0 * (4.0 * std::f64::consts::PI * 40_000.0 / (C / 1090e6)).log10();
        assert!((near - fspl).abs() < 0.5, "{near} vs {fspl}");
        let far: Vec<f64> = vec![0.0; 6001];
        let beyond = path_loss(&far, 100.0, 10.0, 10_000.0, 1090.0, &Params::default());
        let horizon =
            (radio_horizon(10.0, 4.0 / 3.0) + radio_horizon(10_000.0, 4.0 / 3.0)) / 1000.0;
        assert!(horizon < 600.0);
        let fspl_far = 20.0 * (4.0 * std::f64::consts::PI * 600_000.0 / (C / 1090e6)).log10();
        assert!(
            beyond > fspl_far + 10.0,
            "past the {horizon:.0} km horizon: {beyond} vs {fspl_far}"
        );
    }

    #[test]
    fn takeoff_angle_points_down_from_a_hill_to_the_sea() {
        let p = Profile::from_ground(ep(53.0, -9.0, 10.0), ep(53.0, -9.5, 20.0), 4.0 / 3.0, &{
            let mut g = vec![0.0; 201];
            g[0] = 120.0;
            g
        });
        let r = analyse(&p, 162.0, &Params::default());
        assert!(r.takeoff_deg < 0.0 && r.takeoff_deg > -0.5, "{}", r.takeoff_deg);
    }
}
