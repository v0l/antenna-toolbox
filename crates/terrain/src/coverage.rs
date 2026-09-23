use crate::dem::Dem;
use crate::itm::{self, Params};
use crate::path::{EARTH_RADIUS, LatLon};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub site: LatLon,
    pub tx_agl: f64,
    pub rx_agl: f64,
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
}

impl Coverage {
    pub fn loss_at(&self, p: LatLon) -> Option<f32> {
        let d = self.spec.site.distance_to(p);
        if d > self.spec.radius || d < self.bin * 0.5 {
            return None;
        }
        let n = self.spec.radials;
        let r = ((self.spec.site.bearing_to(p) / 360.0 * n as f64).round() as usize) % n;
        let b = ((d / self.bin).round() as usize).clamp(1, self.bins) - 1;
        let v = self.loss[r * self.bins + b];
        v.is_finite().then_some(v)
    }
}

pub fn compute(
    dem: &Dem,
    spec: &Spec,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &AtomicBool,
) -> Result<Coverage, String> {
    let (s, w, n, e) = spec.bounds();
    let sampler = dem.sampler(s, w, n, e)?;
    Ok(compute_with(&|lat, lon| sampler.elevation(lat, lon), spec, progress, cancel)
        .ok_or("cancelled")?)
}

pub fn compute_with(
    elevation: &(dyn Fn(f64, f64) -> f64 + Sync),
    spec: &Spec,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &AtomicBool,
) -> Option<Coverage> {
    let samples = (spec.radius / spec.step).floor() as usize;
    let bins = samples / spec.stride;
    let bin = spec.step * spec.stride as f64;
    let done = AtomicUsize::new(0);
    let rows: Vec<(Vec<f32>, Vec<f32>)> = (0..spec.radials)
        .into_par_iter()
        .map(|r| {
            if cancel.load(Ordering::Relaxed) {
                return (vec![f32::NAN; bins], vec![0.0; bins]);
            }
            let bearing = r as f64 * 360.0 / spec.radials as f64;
            let ground: Vec<f64> = (0..=samples)
                .map(|i| {
                    let q = spec.site.destination(bearing, i as f64 * spec.step);
                    elevation(q.lat, q.lon).max(0.0)
                })
                .collect();
            let mut loss = Vec::with_capacity(bins);
            let mut height = Vec::with_capacity(bins);
            for b in 1..=bins {
                let i = b * spec.stride;
                height.push(ground[i] as f32);
                let l = if i < 2 {
                    f64::NAN
                } else {
                    itm::point_to_point(
                        spec.tx_agl,
                        spec.rx_agl,
                        &ground[..=i],
                        spec.step,
                        spec.freq_mhz,
                        &spec.params,
                    )
                    .map(|x| x.loss_db)
                    .unwrap_or(f64::NAN)
                };
                loss.push(l as f32);
            }
            let k = done.fetch_add(1, Ordering::Relaxed) + 1;
            if k.is_multiple_of(16) {
                progress(k as f32 / spec.radials as f32);
            }
            (loss, height)
        })
        .collect();
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let mut loss = Vec::with_capacity(spec.radials * bins);
    let mut ground = Vec::with_capacity(spec.radials * bins);
    for (l, g) in rows {
        loss.extend(l);
        ground.extend(g);
    }
    Some(Coverage { spec: spec.clone(), bins, bin, loss, ground })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        Spec {
            site: LatLon::new(53.0, -9.0),
            tx_agl: 10.0,
            rx_agl: 10.0,
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
            itm::point_to_point(10.0, 10.0, &vec![20.0; 201], 100.0, 162.0, &s.params).unwrap();
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
    }
}
