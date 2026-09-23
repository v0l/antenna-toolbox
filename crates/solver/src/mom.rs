use crate::geometry::{Blocked, ImagePlane, Mesh, SolveLine, WireGeometry};
use crate::linalg::{Currents, System, solve_system};
use crate::units::ETA;
use crate::vec::{Vec3, dot, length, lerp, scale, sub};
use num_complex::Complex64 as C64;
use rayon::prelude::*;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub a: Vec3,
    pub b: Vec3,
    pub mid: Vec3,
    pub dir: Vec3,
    pub len: f64,
    pub rad: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct Half {
    pub seg: usize,
    pub at_b: bool,
    pub sign: f64,
}

impl Half {
    fn divergence(&self, len: f64) -> f64 {
        self.sign * if self.at_b { 1.0 } else { -1.0 } / len
    }
}

#[derive(Clone, Debug)]
pub struct Basis {
    pub halves: Vec<Half>,
    pub node: Vec3,
}

#[derive(Clone)]
pub struct Model {
    pub segs: Vec<Segment>,
    pub bases: Vec<Basis>,
    pub a: f64,
    pub feed: usize,
    pub feed_point: Vec3,
    pub max_degree: usize,
    pub ground_z: Option<f64>,
    pub images: Vec<ImageTransform>,
    pub blocked: Option<Blocked>,
    pub po: Option<Mesh>,
}

fn split_at_feed(lines: &[SolveLine], feed: Vec3) -> Vec<SolveLine> {
    lines
        .iter()
        .map(|spec| {
            let mut pts = Vec::with_capacity(spec.pts.len() + 1);
            for (i, &p) in spec.pts.iter().enumerate() {
                if i > 0 {
                    let a = spec.pts[i - 1];
                    let ab = sub(p, a);
                    let l = length(ab);
                    if l > 0.0 {
                        let t = dot(sub(feed, a), ab) / (l * l);
                        let off = length(sub(feed, lerp(a, p, t)));
                        let tol = 1e-6 * l;
                        if off < tol && t * l > tol && (1.0 - t) * l > tol {
                            pts.push(feed);
                        }
                    }
                }
                pts.push(p);
            }
            SolveLine { pts, rad: spec.rad }
        })
        .collect()
}

pub fn segmentise(lines: &[SolveLine], lam: f64, cap: usize) -> Vec<Segment> {
    let mut target = lam / 60.0;
    loop {
        let mut segs = Vec::new();
        for spec in lines {
            for pair in spec.pts.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let l = length(sub(b, a));
                if l < 1e-9 {
                    continue;
                }
                let n = ((l / target).round() as usize).max(1);
                for s in 0..n {
                    let p0 = lerp(a, b, s as f64 / n as f64);
                    let p1 = lerp(a, b, (s + 1) as f64 / n as f64);
                    let d = sub(p1, p0);
                    let len = length(d);
                    segs.push(Segment {
                        a: p0,
                        b: p1,
                        len,
                        dir: scale(d, 1.0 / len),
                        mid: lerp(p0, p1, 0.5),
                        rad: spec.rad.map(|r| r.min(0.3 * len)),
                    });
                }
            }
        }
        if segs.len() <= cap || target > lam / 4.0 {
            return segs;
        }
        target *= 1.25;
    }
}

fn node_key(p: Vec3) -> [i64; 3] {
    p.map(|v| (v * 1e3).round() as i64)
}

fn on_plane(p: Vec3, planes: &[ImagePlane]) -> bool {
    planes.iter().any(|pl| (p[pl.axis] - pl.at).abs() < 1e-4)
}

pub fn bases_for(segs: &[Segment], planes: &[ImagePlane]) -> (Vec<Basis>, usize) {
    let mut order: Vec<[i64; 3]> = Vec::new();
    let mut at: HashMap<[i64; 3], (Vec3, Vec<(usize, bool)>)> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        for (p, at_b) in [(s.a, false), (s.b, true)] {
            at.entry(node_key(p))
                .or_insert_with(|| {
                    order.push(node_key(p));
                    (p, Vec::new())
                })
                .1
                .push((i, at_b));
        }
    }
    let outgoing = |seg: usize, at_b: bool| Half { seg, at_b, sign: if at_b { -1.0 } else { 1.0 } };
    let mut bases = Vec::new();
    let mut max_degree = 0;
    for key in order {
        let (node, ends) = &at[&key];
        max_degree = max_degree.max(ends.len());
        if on_plane(*node, planes) {
            for &(seg, at_b) in ends {
                bases.push(Basis { halves: vec![outgoing(seg, at_b)], node: *node });
            }
            continue;
        }
        let (seg0, b0) = ends[0];
        let incoming = Half { seg: seg0, at_b: b0, sign: if b0 { 1.0 } else { -1.0 } };
        for &(seg, at_b) in &ends[1..] {
            bases.push(Basis { halves: vec![incoming, outgoing(seg, at_b)], node: *node });
        }
    }
    (bases, max_degree)
}

pub fn build_model(geo: &WireGeometry, lam: f64, wire_dia: f64, cap: usize) -> Model {
    let lines = split_at_feed(&geo.lines, geo.feed);
    let segs = segmentise(&lines, lam, cap);
    let min_len = segs.iter().map(|s| s.len).fold(f64::INFINITY, f64::min);
    let a = (wire_dia / 2.0).min(0.3 * min_len).max(1e-4 * lam);
    let mut planes: Vec<ImagePlane> = geo.mirrors.clone();
    if let Some(gz) = geo.ground_z {
        planes.push(ImagePlane { axis: 2, at: gz });
    }
    let (bases, max_degree) = bases_for(&segs, &planes);
    let feed = bases
        .iter()
        .enumerate()
        .min_by(|x, y| length(sub(x.1.node, geo.feed)).total_cmp(&length(sub(y.1.node, geo.feed))))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let mut images = Vec::new();
    if let Some(gz) = geo.ground_z {
        images.push(ImageTransform { planes: vec![ImagePlane { axis: 2, at: gz }] });
    }
    if !geo.mirrors.is_empty() {
        images.extend(expand_images(&geo.mirrors));
    }

    Model {
        feed_point: bases.get(feed).map(|b| b.node).unwrap_or(geo.feed),
        segs,
        bases,
        a,
        feed,
        max_degree,
        ground_z: geo.ground_z,
        images,
        blocked: geo.blocked.clone(),
        po: geo.po.clone(),
    }
}

#[derive(Clone, Debug)]
pub struct ImageTransform {
    pub planes: Vec<ImagePlane>,
}

impl ImageTransform {
    pub fn apply(&self, p: Vec3) -> Vec3 {
        let mut q = p;
        for pl in &self.planes {
            q[pl.axis] = 2.0 * pl.at - q[pl.axis];
        }
        q
    }

    pub fn reverses_current(&self) -> bool {
        self.planes.len() % 2 == 1
    }

    fn sign(&self) -> f64 {
        if self.reverses_current() { -1.0 } else { 1.0 }
    }
}

pub fn expand_images(planes: &[ImagePlane]) -> Vec<ImageTransform> {
    (1..1usize << planes.len())
        .map(|mask| ImageTransform {
            planes: planes
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, p)| *p)
                .collect(),
        })
        .collect()
}

const GL3: [(f64, f64); 3] = [
    (0.112_701_665_379_258_3, 5.0 / 18.0),
    (0.5, 8.0 / 18.0),
    (0.887_298_334_620_741_7, 5.0 / 18.0),
];

const GL8: [(f64, f64); 8] = [
    (0.019_855_071_751_231_9, 0.050_614_268_145_188_1),
    (0.101_666_761_293_186_6, 0.111_190_517_226_687_2),
    (0.237_233_795_041_835_5, 0.156_853_322_938_943_6),
    (0.408_282_678_752_175_1, 0.181_341_891_689_181_1),
    (0.591_717_321_247_825, 0.181_341_891_689_181_1),
    (0.762_766_204_958_164_5, 0.156_853_322_938_943_6),
    (0.898_333_238_706_813_4, 0.111_190_517_226_687_2),
    (0.980_144_928_248_768_1, 0.050_614_268_145_188_1),
];

#[derive(Clone, Copy, Default)]
struct PairIntegrals {
    p: [[C64; 2]; 2],
    q: C64,
}

fn smooth_kernel(r: f64, k: f64) -> C64 {
    let kr = k * r;
    let h = (kr / 2.0).sin();
    C64::new(-2.0 * h * h / r, -kr.sin() / r) / (4.0 * PI)
}

fn full_kernel(r: f64, k: f64) -> C64 {
    C64::new((k * r).cos(), -(k * r).sin()) / (4.0 * PI * r)
}

fn pair_integrals(o: &Segment, sa: Vec3, sb: Vec3, s_len: f64, rad: f64, k: f64) -> PairIntegrals {
    let smid = lerp(sa, sb, 0.5);
    let near = length(sub(o.mid, smid)) < 1.5 * (o.len + s_len) + 4.0 * rad;
    let sdir = scale(sub(sb, sa), 1.0 / s_len);
    let mut out = PairIntegrals::default();

    let outer: &[(f64, f64)] = if near { &GL8 } else { &GL3 };
    for &(t, wt) in outer {
        let p = lerp(o.a, o.b, t);
        let wo = [1.0 - t, t];
        let mut inner = [C64::new(0.0, 0.0); 2];
        let mut total = C64::new(0.0, 0.0);
        if near {
            for &(u, wu) in &GL8 {
                let q = lerp(sa, sb, u);
                let d = sub(p, q);
                let r = (dot(d, d) + rad * rad).sqrt();
                let g = smooth_kernel(r, k) * wu * s_len;
                inner[0] += g * (1.0 - u);
                inner[1] += g * u;
                total += g;
            }
            let rel = sub(p, sa);
            let t0 = dot(rel, sdir);
            let rho2 = (dot(rel, rel) - t0 * t0).max(0.0) + rad * rad;
            let rho = rho2.sqrt();
            let r0 = (t0 * t0 + rho2).sqrt();
            let r1 = ((s_len - t0) * (s_len - t0) + rho2).sqrt();
            let i0 = ((s_len - t0) / rho).asinh() + (t0 / rho).asinh();
            let i1 = (r1 - r0) + t0 * i0;
            let c = 1.0 / (4.0 * PI);
            let w1 = c * i1 / s_len;
            let w0 = c * i0 - w1;
            inner[0] += w0;
            inner[1] += w1;
            total += C64::new(c * i0, 0.0);
        } else {
            for &(u, wu) in &GL3 {
                let q = lerp(sa, sb, u);
                let d = sub(p, q);
                let r = (dot(d, d) + rad * rad).sqrt();
                let g = full_kernel(r, k) * wu * s_len;
                inner[0] += g * (1.0 - u);
                inner[1] += g * u;
                total += g;
            }
        }
        let w = wt * o.len;
        for i in 0..2 {
            for j in 0..2 {
                out.p[i][j] += inner[j] * (wo[i] * w);
            }
        }
        out.q += total * w;
    }
    out
}

struct Source {
    a: Vec3,
    b: Vec3,
    dir: Vec3,
    len: f64,
    rad: f64,
    sign: f64,
}

fn sources(segs: &[Segment], a_default: f64, images: &[ImageTransform]) -> Vec<Vec<Source>> {
    let mut all = vec![
        segs.iter()
            .map(|s| Source {
                a: s.a,
                b: s.b,
                dir: s.dir,
                len: s.len,
                rad: s.rad.unwrap_or(a_default),
                sign: 1.0,
            })
            .collect(),
    ];
    for t in images {
        all.push(
            segs.iter()
                .map(|s| {
                    let (a, b) = (t.apply(s.a), t.apply(s.b));
                    Source {
                        a,
                        b,
                        dir: scale(sub(b, a), 1.0 / s.len),
                        len: s.len,
                        rad: s.rad.unwrap_or(a_default),
                        sign: t.sign(),
                    }
                })
                .collect(),
        );
    }
    all
}

pub fn fill(model: &Model, k: f64) -> System {
    let segs = &model.segs;
    let bases = &model.bases;
    let n = bases.len();
    let mut sys = System::zeros(n);
    let w = sys.w;
    let groups = sources(segs, model.a, &model.images);
    let jk_eta = C64::new(0.0, k * ETA);
    let j_eta_k = C64::new(0.0, ETA / k);

    sys.a.par_chunks_mut(w).enumerate().for_each(|(m, row)| {
        let bm = &bases[m];
        for group in &groups {
            let rows: Vec<Vec<PairIntegrals>> = bm
                .halves
                .iter()
                .map(|h| {
                    let o = &segs[h.seg];
                    group.iter().map(|s| pair_integrals(o, s.a, s.b, s.len, s.rad, k)).collect()
                })
                .collect();
            for (ni, bn) in bases.iter().enumerate() {
                let mut z = C64::new(0.0, 0.0);
                for (hi, h) in bm.halves.iter().enumerate() {
                    let o = &segs[h.seg];
                    let dm = h.divergence(o.len);
                    for g in &bn.halves {
                        let s = &group[g.seg];
                        let pi = &rows[hi][g.seg];
                        let sig = h.sign * g.sign * s.sign;
                        let pv = pi.p[h.at_b as usize][g.at_b as usize];
                        let dn = g.divergence(s.len) * s.sign;
                        z += jk_eta * pv * (dot(o.dir, s.dir) * sig) - j_eta_k * pi.q * (dm * dn);
                    }
                }
                row[ni] += z;
            }
        }
    });
    sys
}

pub fn solve_cpu(model: &Model, k: f64) -> Currents {
    let mut sys = fill(model, k);
    if !model.bases.is_empty() {
        *sys.rhs_mut(model.feed) = C64::new(1.0, 0.0);
    }
    solve_system(sys)
}

pub fn segment_currents(model: &Model, coeffs: &[C64]) -> Currents {
    let mut out = vec![C64::new(0.0, 0.0); model.segs.len()];
    for (b, &c) in model.bases.iter().zip(coeffs) {
        for h in &b.halves {
            out[h.seg] += c * (0.5 * h.sign);
        }
    }
    out
}

pub type Pattern = Arc<dyn Fn(Vec3) -> f64 + Send + Sync>;
pub type FieldFn = Arc<dyn Fn(Vec3) -> (Vec3, Vec3) + Send + Sync>;

pub fn transverse(fr: Vec3, fi: Vec3, dir: Vec3) -> (Vec3, Vec3) {
    let pr = dot(fr, dir);
    let pi = dot(fi, dir);
    (sub(fr, scale(dir, pr)), sub(fi, scale(dir, pi)))
}

pub fn magnitude(er: Vec3, ei: Vec3) -> f64 {
    (dot(er, er) + dot(ei, ei)).sqrt()
}

fn accumulate(fr: &mut Vec3, fi: &mut Vec3, cur: C64, len: f64, ph: f64, dir: Vec3) {
    let w = cur * C64::new(ph.cos(), ph.sin()) * len;
    for t in 0..3 {
        fr[t] += w.re * dir[t];
        fi[t] += w.im * dir[t];
    }
}

pub fn far_field_vector(
    segs: Arc<Vec<Segment>>,
    cur: Arc<Currents>,
    k: f64,
    gz: Option<f64>,
    images: Vec<ImageTransform>,
    blocked: Option<Blocked>,
) -> FieldFn {
    Arc::new(move |dir: Vec3| {
        let zero = ([0.0; 3], [0.0; 3]);
        if gz.is_some() && dir[2] <= 0.0 {
            return zero;
        }
        if blocked.as_ref().is_some_and(|b| b(dir)) {
            return zero;
        }
        let mut fr = [0.0; 3];
        let mut fi = [0.0; 3];
        for (n, s) in segs.iter().enumerate() {
            accumulate(&mut fr, &mut fi, cur[n], s.len, k * dot(dir, s.mid), s.dir);
            for t in &images {
                let im = t.apply(s.mid);
                let mut refl = s.dir;
                for pl in &t.planes {
                    refl[pl.axis] = -refl[pl.axis];
                }
                let idir = if t.reverses_current() { scale(refl, -1.0) } else { refl };
                accumulate(&mut fr, &mut fi, cur[n], s.len, k * dot(dir, im), idir);
            }
        }
        transverse(fr, fi, dir)
    })
}

pub fn pattern_of(field: FieldFn) -> Pattern {
    Arc::new(move |dir| {
        let (er, ei) = field(dir);
        magnitude(er, ei)
    })
}

pub struct Directivity {
    pub linear: f64,
    pub peak: f64,
}

pub fn directivity(f: &(dyn Fn(Vec3) -> f64 + Sync)) -> Directivity {
    let (nt, np) = (90usize, 120usize);
    let (total, max) = (0..nt)
        .into_par_iter()
        .map(|i| {
            let th = (i as f64 + 0.5) / nt as f64 * PI;
            let (st, ct) = th.sin_cos();
            let mut total = 0.0;
            let mut max: f64 = 0.0;
            for j in 0..np {
                let ph = (j as f64 + 0.5) / np as f64 * 2.0 * PI;
                let u = f([st * ph.cos(), ct, st * ph.sin()]);
                let p = u * u;
                max = max.max(p);
                total += p * st;
            }
            (total, max)
        })
        .reduce(|| (0.0, 0.0), |a, b| (a.0 + b.0, a.1.max(b.1)));
    let total = total * (PI / nt as f64) * (2.0 * PI / np as f64);
    Directivity { linear: if total > 0.0 { 4.0 * PI * max / total } else { 0.0 }, peak: max.sqrt() }
}

pub fn radiated_power(f: &(dyn Fn(Vec3) -> f64 + Sync), k: f64) -> f64 {
    let (nt, np) = (90usize, 120usize);
    let total: f64 = (0..nt)
        .into_par_iter()
        .map(|i| {
            let th = (i as f64 + 0.5) / nt as f64 * PI;
            let (st, ct) = th.sin_cos();
            (0..np)
                .map(|j| {
                    let ph = (j as f64 + 0.5) / np as f64 * 2.0 * PI;
                    let u = f([st * ph.cos(), ct, st * ph.sin()]);
                    u * u * st
                })
                .sum::<f64>()
        })
        .sum();
    let integral = total * (PI / nt as f64) * (2.0 * PI / np as f64);
    ETA * k * k / (32.0 * PI * PI) * integral
}
