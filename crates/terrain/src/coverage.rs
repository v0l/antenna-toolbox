use crate::dem::Dem;
use crate::itm::Params;
use crate::path::{EARTH_RADIUS, LatLon, arrival_angle, path_loss};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub site: LatLon,
    pub tx_agl: f64,
    pub rx_agl: f64,
    pub rx_above_sea: bool,
    pub radius: f64,
    pub freq_mhz: f64,
    pub params: Params,
    pub radials: usize,
    pub step: f64,
    pub stride: usize,
}

impl Spec {
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let dlat = (self.radius / EARTH_RADIUS).to_degrees();
        let dlon = dlat / self.site.lat.to_radians().cos().max(0.05);
        (self.site.lat - dlat, self.site.lon - dlon, self.site.lat + dlat, self.site.lon + dlon)
    }
}

#[derive(Clone, Debug)]
pub struct Coverage {
    pub spec: Spec,
    pub bins: usize,
    pub bin: f64,
    pub loss: Vec<f32>,
    pub ground: Vec<f32>,
    pub takeoff: Vec<f32>,
    pub arrival: Vec<f32>,
}

impl Coverage {
    fn index(&self, p: LatLon) -> Option<usize> {
        let d = self.spec.site.distance_to(p);
        if d > self.spec.radius || d < self.bin * 0.5 {
            return None;
        }
        let n = self.spec.radials;
        let r = ((self.spec.site.bearing_to(p) / 360.0 * n as f64).round() as usize) % n;
        let b = ((d / self.bin).round() as usize).clamp(1, self.bins) - 1;
        Some(r * self.bins + b)
    }

    pub fn loss_at(&self, p: LatLon) -> Option<f32> {
        self.index(p).map(|i| self.loss[i]).filter(|v| v.is_finite())
    }

    pub fn takeoff_at(&self, p: LatLon) -> Option<f32> {
        self.index(p).map(|i| self.takeoff[i])
    }

    pub fn arrival_at(&self, p: LatLon) -> Option<f32> {
        self.index(p).map(|i| self.arrival[i])
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Stage {
    Tiles(usize, usize),
    Radials(f32),
}

pub fn compute(
    dem: &Dem,
    spec: &Spec,
    progress: &(dyn Fn(Stage) + Sync),
    cancel: &AtomicBool,
) -> Result<Coverage, String> {
    let (s, w, n, e) = spec.bounds();
    let sampler = dem.sampler(s, w, n, e, &|k, of| progress(Stage::Tiles(k, of)))?;
    let radials = |f: f32| progress(Stage::Radials(f));
    Ok(compute_with(&|lat, lon| sampler.elevation(lat, lon), spec, &radials, cancel)
        .ok_or("cancelled")?)
}

pub fn stride_for(radius: f64, step: f64) -> usize {
    ((radius / step / 1500.0).ceil() as usize).max(2)
}

pub struct Grid {
    samples: usize,
    pub bins: usize,
    pub bin: f64,
}

impl Grid {
    pub fn of(spec: &Spec) -> Grid {
        let samples = (spec.radius / spec.step).floor() as usize;
        Grid { samples, bins: samples / spec.stride, bin: spec.step * spec.stride as f64 }
    }
}

pub type Radial = [Vec<f32>; 4];

pub fn radial(elevation: &(dyn Fn(f64, f64) -> f64 + Sync), spec: &Spec, r: usize) -> Radial {
    let Grid { samples, bins, .. } = Grid::of(spec);
    let bearing = r as f64 * 360.0 / spec.radials as f64;
    let ground: Vec<f64> = (0..=samples)
        .map(|i| {
            let q = spec.site.destination(bearing, i as f64 * spec.step);
            elevation(q.lat, q.lon).max(0.0)
        })
        .collect();
    let mut loss = Vec::with_capacity(bins);
    let mut height = Vec::with_capacity(bins);
    let mut takeoff = Vec::with_capacity(bins);
    let mut arrival = Vec::with_capacity(bins);
    let curve = 2.0 * (4.0 / 3.0) * EARTH_RADIUS;
    let tx_abs = ground[0] + spec.tx_agl;
    let angle = |j: usize, top: f64| {
        let d = j as f64 * spec.step;
        ((top - tx_abs) / d - d / curve).atan()
    };
    let mut horizon = f64::NEG_INFINITY;
    let mut seen = 1;
    for b in 1..=bins {
        let i = b * spec.stride;
        height.push(ground[i] as f32);
        while seen < i {
            horizon = horizon.max(angle(seen, ground[seen]));
            seen += 1;
        }
        let rx = if spec.rx_above_sea { (spec.rx_agl - ground[i]).max(0.5) } else { spec.rx_agl };
        let l = if i < 2 {
            f64::NAN
        } else {
            path_loss(&ground[..=i], spec.step, spec.tx_agl, rx, spec.freq_mhz, &spec.params)
        };
        loss.push(l as f32);
        takeoff.push(angle(i, ground[i] + rx).max(horizon).to_degrees() as f32);
        arrival.push(if i < 2 {
            0.0
        } else {
            arrival_angle(&ground[..=i], spec.step, spec.tx_agl, rx, 4.0 / 3.0) as f32
        });
    }
    [loss, height, takeoff, arrival]
}

pub fn assemble(spec: &Spec, rows: Vec<Radial>) -> Coverage {
    let Grid { bins, bin, .. } = Grid::of(spec);
    let mut loss = Vec::with_capacity(spec.radials * bins);
    let mut ground = Vec::with_capacity(spec.radials * bins);
    let mut takeoff = Vec::with_capacity(spec.radials * bins);
    let mut arrival = Vec::with_capacity(spec.radials * bins);
    for [l, g, t, a] in rows {
        loss.extend(l);
        ground.extend(g);
        takeoff.extend(t);
        arrival.extend(a);
    }
    Coverage { spec: spec.clone(), bins, bin, loss, ground, takeoff, arrival }
}

pub fn compute_with(
    elevation: &(dyn Fn(f64, f64) -> f64 + Sync),
    spec: &Spec,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &AtomicBool,
) -> Option<Coverage> {
    let bins = Grid::of(spec).bins;
    let done = AtomicUsize::new(0);
    let rows: Vec<Radial> = (0..spec.radials)
        .into_par_iter()
        .map(|r| {
            if cancel.load(Ordering::Relaxed) {
                return [vec![f32::NAN; bins], vec![0.0; bins], vec![0.0; bins], vec![0.0; bins]];
            }
            let row = radial(elevation, spec, r);
            let k = done.fetch_add(1, Ordering::Relaxed) + 1;
            if k.is_multiple_of(16) {
                progress(k as f32 / spec.radials as f32);
            }
            row
        })
        .collect();
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    Some(assemble(spec, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        Spec {
            site: LatLon::new(53.0, -9.0),
            tx_agl: 10.0,
            rx_agl: 10.0,
            rx_above_sea: false,
            radius: 30e3,
            freq_mhz: 162.0,
            params: Params::default(),
            radials: 72,
            step: 100.0,
            stride: 5,
        }
    }

    #[test]
    fn a_flat_plain_loses_more_with_distance_and_matches_a_single_path() {
        let s = spec();
        let c = compute_with(&|_, _| 20.0, &s, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(c.bins, 60);
        let row = &c.loss[..c.bins];
        assert!(row.windows(2).all(|w| w[1] >= w[0] - 0.5), "{row:?}");
        let far = s.site.destination(90.0, 20e3);
        let direct =
            crate::itm::point_to_point(10.0, 10.0, &vec![20.0; 201], 100.0, 162.0, &s.params)
                .unwrap();
        let got = c.loss_at(far).unwrap() as f64;
        assert!((got - direct.loss_db).abs() < 0.01, "{got} vs {}", direct.loss_db);
        assert!(c.loss_at(s.site.destination(0.0, 31e3)).is_none());
    }

    #[test]
    fn a_ridge_to_the_north_shadows_only_the_north() {
        let s = spec();
        let site = s.site;
        let ridge = move |lat: f64, lon: f64| {
            let p = LatLon::new(lat, lon);
            let (d, b) = (site.distance_to(p), site.bearing_to(p));
            if (4e3..5e3).contains(&d) && !(30.0..330.0).contains(&b) { 300.0 } else { 20.0 }
        };
        let c = compute_with(&ridge, &s, &|_| {}, &AtomicBool::new(false)).unwrap();
        let north = c.loss_at(site.destination(0.0, 15e3)).unwrap();
        let south = c.loss_at(site.destination(180.0, 15e3)).unwrap();
        assert!(north - south > 20.0, "north {north} south {south}");
        let over_ridge = c.takeoff_at(site.destination(0.0, 15e3)).unwrap();
        let open = c.takeoff_at(site.destination(180.0, 15e3)).unwrap();
        let ridge_angle = ((300.0 - 30.0) / 4000.0f64).atan().to_degrees() as f32;
        assert!((over_ridge - ridge_angle).abs() < 0.3, "{over_ridge} vs {ridge_angle}");
        assert!(open < 0.0 && open > -0.2, "{open}");
    }
}
