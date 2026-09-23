use crate::geometry::Mesh;
use crate::linalg::Currents;
use crate::mom::{FieldFn, Segment, transverse};
use crate::vec::{Vec3, add, cross, dot, length, scale, sub};
use num_complex::Complex64 as C64;
use rayon::prelude::*;
use std::f64::consts::PI;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct PoFacet {
    pub centre: Vec3,
    pub area: f64,
    pub normal: Vec3,
    pub jr: Vec3,
    pub ji: Vec3,
}

fn incident_h(p: Vec3, segs: &[Segment], cur: &Currents, k: f64) -> (Vec3, Vec3) {
    let mut hr = [0.0; 3];
    let mut hi = [0.0; 3];
    for (n, s) in segs.iter().enumerate() {
        let d = sub(p, s.mid);
        let r = length(d).max(1e-9);
        let cr = cross(s.dir, scale(d, 1.0 / r));
        let g = C64::new(1.0 / (r * r), k / r) * C64::new((k * r).cos(), -(k * r).sin());
        let w = cur[n] * s.len * g / (4.0 * PI);
        for t in 0..3 {
            hr[t] += w.re * cr[t];
            hi[t] += w.im * cr[t];
        }
    }
    (hr, hi)
}

pub fn po_currents(
    mesh: &Mesh,
    segs: &[Segment],
    cur: &Currents,
    k: f64,
    source: Vec3,
) -> Vec<PoFacet> {
    mesh.triangles
        .par_iter()
        .filter_map(|t| {
            let (a, b, c) = (mesh.vertices[t[0]], mesh.vertices[t[1]], mesh.vertices[t[2]]);
            let n = cross(sub(b, a), sub(c, a));
            let twice = length(n);
            if twice < 1e-12 {
                return None;
            }
            let mut normal = scale(n, 1.0 / twice);
            let centre = scale(add(add(a, b), c), 1.0 / 3.0);
            let to_source = sub(source, centre);
            if dot(normal, to_source) < 0.0 {
                normal = scale(normal, -1.0);
            }
            if dot(normal, to_source) <= 0.0 {
                return None;
            }
            let (hr, hi) = incident_h(centre, segs, cur, k);
            Some(PoFacet {
                centre,
                area: twice / 2.0,
                normal,
                jr: scale(cross(normal, hr), 2.0),
                ji: scale(cross(normal, hi), 2.0),
            })
        })
        .collect()
}

pub fn free_field_vector(segs: Arc<Vec<Segment>>, cur: Arc<Currents>, k: f64) -> FieldFn {
    Arc::new(move |dir| {
        let (fr, fi) = raw_sum(&segs, &cur, k, dir);
        transverse(fr, fi, dir)
    })
}

fn raw_sum(segs: &[Segment], cur: &Currents, k: f64, dir: Vec3) -> (Vec3, Vec3) {
    let mut fr = [0.0; 3];
    let mut fi = [0.0; 3];
    for (n, s) in segs.iter().enumerate() {
        let ph = k * dot(dir, s.mid);
        let w = cur[n] * C64::new(ph.cos(), ph.sin()) * s.len;
        for t in 0..3 {
            fr[t] += w.re * s.dir[t];
            fi[t] += w.im * s.dir[t];
        }
    }
    (fr, fi)
}

pub fn hybrid_field(
    segs: Arc<Vec<Segment>>,
    cur: Arc<Currents>,
    facets: Arc<Vec<PoFacet>>,
    k: f64,
) -> FieldFn {
    Arc::new(move |dir| {
        let (mut fr, mut fi) = raw_sum(&segs, &cur, k, dir);
        for f in facets.iter() {
            let ph = k * dot(dir, f.centre);
            let (s, c) = ph.sin_cos();
            for t in 0..3 {
                let jr = f.jr[t] * f.area;
                let ji = f.ji[t] * f.area;
                fr[t] += jr * c - ji * s;
                fi[t] += jr * s + ji * c;
            }
        }
        transverse(fr, fi, dir)
    })
}
