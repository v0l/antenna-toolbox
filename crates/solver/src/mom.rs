use crate::geometry::{Blocked, ImagePlane, Mesh, RealGround, SolveLine, WireGeometry, WireProps};
use crate::linalg::{Currents, System, solve_system};
use crate::units::ETA;
use crate::vec::{Vec3, cross, dot, length, lerp, scale, sub};
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
    pub thinned: bool,
    pub props: WireProps,
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
    pub sources: Vec<(usize, C64)>,
    pub loads: Vec<(usize, crate::geometry::Load)>,
    pub networks: Vec<(usize, usize, crate::geometry::Network)>,
    pub max_degree: usize,
    pub ground_z: Option<f64>,
    pub real_ground: Option<RealGround>,
    pub images: Vec<ImageTransform>,
    pub blocked: Option<Blocked>,
    pub po: Option<Mesh>,
}

fn split_at(lines: &[SolveLine], ports: &[Vec3]) -> Vec<SolveLine> {
    lines
        .iter()
        .map(|spec| {
            let mut pts = Vec::with_capacity(spec.pts.len() + ports.len());
            for (i, &p) in spec.pts.iter().enumerate() {
                if i > 0 {
                    let a = spec.pts[i - 1];
                    let ab = sub(p, a);
                    let l = length(ab);
                    if l > 0.0 {
                        let tol = 1e-6 * l;
                        let mut inside: Vec<(f64, Vec3)> = ports
                            .iter()
                            .filter_map(|&q| {
                                let t = dot(sub(q, a), ab) / (l * l);
                                let off = length(sub(q, lerp(a, p, t)));
                                (off < tol && t * l > tol && (1.0 - t) * l > tol).then_some((t, q))
                            })
                            .collect();
                        inside.sort_by(|x, y| x.0.total_cmp(&y.0));
                        inside.dedup_by(|x, y| (x.0 - y.0).abs() * l < tol);
                        pts.extend(inside.into_iter().map(|(_, q)| q));
                    }
                }
                pts.push(p);
            }
            let segments = spec.segments.filter(|_| pts.len() == spec.pts.len());
            SolveLine { pts, segments, ..spec.clone() }
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
                let n = spec.segments.unwrap_or(((l / target).round() as usize).max(1)).max(1);
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
                        thinned: spec.rad.is_some_and(|r| r > 0.3 * len),
                        props: spec.props,
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

type NodeEnds = (Vec3, Vec<(usize, bool)>);

pub fn bases_for(segs: &[Segment], planes: &[ImagePlane]) -> (Vec<Basis>, usize) {
    let mut order: Vec<[i64; 3]> = Vec::new();
    let mut at: HashMap<[i64; 3], NodeEnds> = HashMap::new();
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
    let mut ports = vec![geo.feed];
    ports.extend(geo.sources.iter().map(|s| s.0));
    ports.extend(geo.loads.iter().map(|l| l.0));
    ports.extend(geo.networks.iter().flat_map(|n| [n.0, n.1]));
    let lines = split_at(&geo.lines, &ports);
    let segs = segmentise(&lines, lam, cap);
    let min_len = segs.iter().map(|s| s.len).fold(f64::INFINITY, f64::min);
    let a = (wire_dia / 2.0).min(0.3 * min_len).max(1e-4 * lam);
    let mut planes: Vec<ImagePlane> = geo.mirrors.clone();
    if let Some(gz) = geo.ground_z {
        planes.push(ImagePlane { axis: 2, at: gz });
    }
    let (bases, max_degree) = bases_for(&segs, &planes);
    let nearest = |p: Vec3| {
        bases
            .iter()
            .enumerate()
            .min_by(|x, y| length(sub(x.1.node, p)).total_cmp(&length(sub(y.1.node, p))))
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let feed = nearest(geo.feed);
    let sense = |b: usize| {
        if bases.get(b).and_then(|x| x.halves.first()).is_none_or(|h| h.at_b) { 1.0 } else { -1.0 }
    };
    let fs = sense(feed);
    let sources = geo
        .sources
        .iter()
        .map(|&(p, v)| {
            let b = nearest(p);
            (b, v * sense(b) * fs)
        })
        .collect();
    let loads = geo.loads.iter().map(|&(p, l)| (nearest(p), l)).collect();
    let networks = geo
        .networks
        .iter()
        .map(|&(a, b, n)| {
            let (ba, bb) = (nearest(a), nearest(b));
            let n = if sense(ba) * sense(bb) < 0.0 { flip(n) } else { n };
            (ba, bb, n)
        })
        .collect();

    let mut images = Vec::new();
    if let Some(gz) = geo.ground_z {
        images.push(ImageTransform { planes: vec![ImagePlane { axis: 2, at: gz }] });
    }
    if !geo.mirrors.is_empty() {
        images.extend(expand_images(&geo.mirrors));
    }

    Model {
        feed_point: bases.get(feed).map(|b| b.node).unwrap_or(geo.feed),
        sources,
        loads,
        networks,
        segs,
        bases,
        a,
        feed,
        max_degree,
        ground_z: geo.ground_z,
        real_ground: geo.ground_z.and(geo.real_ground),
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
        for (row, &wi) in out.p.iter_mut().zip(&wo) {
            for (cell, &g) in row.iter_mut().zip(&inner) {
                *cell += g * (wi * w);
            }
        }
        out.q += total * w;
    }
    out
}

struct Source {
    a: Vec3,
    b: Vec3,
    mid: Vec3,
    dir: Vec3,
    len: f64,
    rad: f64,
    sign: f64,
}

struct Group {
    sources: Vec<Source>,
    ground: bool,
}

fn sources(
    segs: &[Segment],
    a_default: f64,
    images: &[ImageTransform],
    gz: Option<f64>,
) -> Vec<Group> {
    let mut all = vec![Group {
        sources: segs
            .iter()
            .map(|s| Source {
                a: s.a,
                b: s.b,
                mid: s.mid,
                dir: s.dir,
                len: s.len,
                rad: s.rad.unwrap_or(a_default),
                sign: 1.0,
            })
            .collect(),
        ground: false,
    }];
    for t in images {
        let ground =
            gz.is_some_and(|z| t.planes.len() == 1 && t.planes[0].axis == 2 && t.planes[0].at == z);
        all.push(Group {
            sources: segs
                .iter()
                .map(|s| {
                    let (a, b) = (t.apply(s.a), t.apply(s.b));
                    Source {
                        a,
                        b,
                        mid: lerp(a, b, 0.5),
                        dir: scale(sub(b, a), 1.0 / s.len),
                        len: s.len,
                        rad: s.rad.unwrap_or(a_default),
                        sign: t.sign(),
                    }
                })
                .collect(),
            ground,
        });
    }
    all
}

pub fn complex_permittivity(g: RealGround, k: f64) -> C64 {
    let lam_m = 2.0 * PI / k / 1000.0;
    C64::new(g.eps_r, -60.0 * g.sigma * lam_m)
}

pub fn fresnel(eps: C64, sin_psi: f64) -> (C64, C64) {
    let cos2 = 1.0 - sin_psi * sin_psi;
    let root = (eps - cos2).sqrt();
    let rv = (eps * sin_psi - root) / (eps * sin_psi + root);
    let rh = (sin_psi - root) / (sin_psi + root);
    (rv, rh)
}

fn surface_impedance(props: &WireProps, radius: f64, k: f64) -> C64 {
    let Some(sigma) = props.conductivity else {
        return C64::new(0.0, 0.0);
    };
    let omega = k * 1000.0 * 299_792_458.0;
    let mu0 = 4.0e-7 * PI;
    let rs = (omega * mu0 / (2.0 * sigma)).sqrt();
    C64::new(rs, rs) / (2.0 * PI * radius / 1000.0) / 1000.0
}

fn insulation_log(props: &WireProps, radius: f64) -> C64 {
    props.insulation.map_or(C64::new(0.0, 0.0), |ins| {
        let inner = ins.inner.max(radius);
        let outer = ins.outer.max(inner);
        let eps = C64::new(ins.eps_r.max(1.0), -ins.eps_r.max(1.0) * ins.tan_d.max(0.0));
        (1.0 - eps.inv()) * (outer / inner).ln()
    })
}

pub fn fill(model: &Model, k: f64) -> System {
    let segs = &model.segs;
    let bases = &model.bases;
    let n = bases.len();
    let mut sys = System::zeros(n);
    let w = sys.w;
    let groups = sources(segs, model.a, &model.images, model.ground_z);
    let jk_eta = C64::new(0.0, k * ETA);
    let j_eta_k = C64::new(0.0, ETA / k);
    let eps = model.real_ground.map(|g| complex_permittivity(g, k));
    let local: Vec<(C64, C64)> = segs
        .iter()
        .map(|s| {
            let r = s.rad.unwrap_or(model.a);
            (surface_impedance(&s.props, r, k), insulation_log(&s.props, r) / (2.0 * PI))
        })
        .collect();

    sys.a.par_chunks_mut(w).enumerate().for_each(|(m, row)| {
        let bm = &bases[m];
        for (gi, group) in groups.iter().enumerate() {
            let real = eps.filter(|_| group.ground);
            let rows: Vec<Vec<PairIntegrals>> = bm
                .halves
                .iter()
                .map(|h| {
                    let o = &segs[h.seg];
                    group
                        .sources
                        .iter()
                        .map(|s| pair_integrals(o, s.a, s.b, s.len, s.rad, k))
                        .collect()
                })
                .collect();
            for (ni, bn) in bases.iter().enumerate() {
                let mut z = C64::new(0.0, 0.0);
                for (hi, h) in bm.halves.iter().enumerate() {
                    let o = &segs[h.seg];
                    let dm = h.divergence(o.len);
                    for g in &bn.halves {
                        let s = &group.sources[g.seg];
                        let pi = &rows[hi][g.seg];
                        let pv = pi.p[h.at_b as usize][g.at_b as usize];
                        let dn = g.divergence(s.len) * s.sign;
                        let sig = h.sign * g.sign * s.sign;
                        match real {
                            None => {
                                z += jk_eta * pv * (dot(o.dir, s.dir) * sig)
                                    - j_eta_k * pi.q * (dm * dn);
                            }
                            Some(eps) => {
                                let d = sub(o.mid, s.mid);
                                let sin_psi = d[2].abs() / length(d).max(1e-12);
                                let (rv, rh) = fresnel(eps, sin_psi);
                                let e = scale(s.dir, sig);
                                let perp = cross([0.0, 0.0, 1.0], d);
                                let pl = length(perp);
                                let (along, across) = if pl > 1e-9 * length(d) {
                                    let u = scale(perp, 1.0 / pl);
                                    let o_perp = dot(o.dir, u);
                                    let across = o_perp * dot(e, u);
                                    (dot(o.dir, e) - across, across)
                                } else {
                                    (dot(o.dir, e), 0.0)
                                };
                                z += jk_eta * pv * (rv * along - rh * across)
                                    - j_eta_k * pi.q * rv * (dm * dn);
                            }
                        }
                    }
                }
                if gi == 0 {
                    for h in &bm.halves {
                        let (zs, ins) = local[h.seg];
                        let o = &segs[h.seg];
                        for g in bn.halves.iter().filter(|g| g.seg == h.seg) {
                            let overlap = if h.at_b == g.at_b { 1.0 / 3.0 } else { 1.0 / 6.0 };
                            z += zs * (h.sign * g.sign * overlap * o.len);
                            z +=
                                j_eta_k * (ins * h.divergence(o.len) * g.divergence(o.len) * o.len);
                        }
                    }
                }
                row[ni] += z;
            }
        }
    });
    sys
}

fn flip(n: crate::geometry::Network) -> crate::geometry::Network {
    use crate::geometry::Network;
    match n {
        Network::Line { z0, length, crossed, shunt } => {
            Network::Line { z0, length, crossed: !crossed, shunt }
        }
        Network::Admittance { y11, y12, y22 } => Network::Admittance { y11, y12: -y12, y22 },
    }
}

impl Model {
    pub fn freq_hz(k: f64) -> f64 {
        k * 1000.0 * 299_792_458.0 / (2.0 * PI)
    }

    pub fn ports(&self) -> Vec<usize> {
        let mut ports: Vec<usize> = self.networks.iter().flat_map(|n| [n.0, n.1]).collect();
        ports.sort_unstable();
        ports.dedup();
        ports
    }

    fn port_admittance(&self, k: f64, ports: &[usize]) -> Vec<C64> {
        let p = ports.len();
        let mut y = vec![C64::new(0.0, 0.0); p * p];
        let at = |b: usize| ports.iter().position(|&x| x == b).unwrap_or(0);
        for &(a, b, n) in &self.networks {
            let m = n.y(Self::freq_hz(k));
            let (i, j) = (at(a), at(b));
            y[i * p + i] += m[0][0];
            y[i * p + j] += m[0][1];
            y[j * p + i] += m[1][0];
            y[j * p + j] += m[1][1];
        }
        y
    }

    pub fn source_current(&self, x: &[C64], b: usize, k: f64) -> C64 {
        let i = x.get(b).copied().unwrap_or_default();
        let ports = self.ports();
        let Some(r) = ports.iter().position(|&p| p == b) else {
            return i;
        };
        let n = self.bases.len();
        let y = self.port_admittance(k, &ports);
        let p = ports.len();
        i + (0..p).map(|j| y[r * p + j] * x.get(n + j).copied().unwrap_or_default()).sum::<C64>()
    }
}

pub fn solve_cpu(model: &Model, k: f64) -> Currents {
    let mut sys = fill(model, k);
    if model.bases.is_empty() {
        return solve_system(sys);
    }
    let mut drive = vec![(model.feed, C64::new(1.0, 0.0))];
    drive.extend(model.sources.iter().copied());
    for &(b, v) in &drive {
        *sys.rhs_mut(b) += v;
    }
    let freq_hz = Model::freq_hz(k);
    for &(b, load) in &model.loads {
        let w = sys.w;
        sys.a[b * w + b] += load.impedance(freq_hz);
    }
    if model.networks.is_empty() {
        return solve_system(sys);
    }
    let ports = model.ports();
    let y = model.port_admittance(k, &ports);
    let (n, p) = (sys.n, ports.len());
    let mut big = crate::linalg::System::zeros(n + p);
    let bw = big.w;
    for r in 0..n {
        big.a[r * bw..r * bw + n].copy_from_slice(&sys.a[r * sys.w..r * sys.w + n]);
        big.a[r * bw + n + p] = sys.a[r * sys.w + n];
    }
    for (k, &b) in ports.iter().enumerate() {
        let row = n + k;
        let driven: C64 = drive.iter().filter(|d| d.0 == b).map(|d| d.1).sum();
        if drive.iter().any(|d| d.0 == b) {
            big.a[b * bw + n + p] -= driven;
            big.a[row * bw + n + k] = C64::new(1.0, 0.0);
            big.a[row * bw + n + p] = driven;
        } else {
            big.a[row * bw + b] = C64::new(1.0, 0.0);
            for j in 0..p {
                big.a[row * bw + n + j] = y[k * p + j];
            }
        }
        big.a[b * bw + n + k] = C64::new(-1.0, 0.0);
    }
    solve_system(big)
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
    real: Option<C64>,
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
        let mut gr = [0.0; 3];
        let mut gi = [0.0; 3];
        for (n, s) in segs.iter().enumerate() {
            accumulate(&mut fr, &mut fi, cur[n], s.len, k * dot(dir, s.mid), s.dir);
            for t in &images {
                let im = t.apply(s.mid);
                let mut refl = s.dir;
                for pl in &t.planes {
                    refl[pl.axis] = -refl[pl.axis];
                }
                let idir = if t.reverses_current() { scale(refl, -1.0) } else { refl };
                let is_ground = real.is_some()
                    && gz.is_some_and(|z| {
                        t.planes.len() == 1 && t.planes[0].axis == 2 && t.planes[0].at == z
                    });
                if is_ground {
                    accumulate(&mut gr, &mut gi, cur[n], s.len, k * dot(dir, im), idir);
                } else {
                    accumulate(&mut fr, &mut fi, cur[n], s.len, k * dot(dir, im), idir);
                }
            }
        }
        if let Some(eps) = real {
            let (rv, rh) = fresnel(eps, dir[2].clamp(0.0, 1.0));
            let horiz = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
            let phi_hat =
                if horiz > 1e-9 { [-dir[1] / horiz, dir[0] / horiz, 0.0] } else { [0.0, 1.0, 0.0] };
            let theta_hat = cross(phi_hat, dir);
            let g = [0, 1, 2].map(|t| C64::new(gr[t], gi[t]));
            let gt: C64 = (0..3).map(|t| g[t] * theta_hat[t]).sum::<C64>() * rv;
            let gp: C64 = (0..3).map(|t| g[t] * phi_hat[t]).sum::<C64>() * (-rh);
            for t in 0..3 {
                let v = gt * theta_hat[t] + gp * phi_hat[t];
                fr[t] += v.re;
                fi[t] += v.im;
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
