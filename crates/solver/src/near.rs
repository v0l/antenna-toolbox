use crate::mom::{ImageTransform, Model, fresnel};
use crate::units::ETA;
use crate::vec::{Vec3, cross, dot, length, lerp, scale, sub};
use num_complex::Complex64 as C64;
use std::f64::consts::PI;

type CVec = [C64; 3];

#[derive(Clone, Copy)]
struct Element {
    a: Vec3,
    b: Vec3,
    len: f64,
    rad: f64,
    ia: CVec,
    ib: CVec,
    q: C64,
    reflected: bool,
}

#[derive(Clone)]
pub struct NearSource {
    elements: Vec<Element>,
    k: f64,
    ground: Option<(f64, C64)>,
    real: usize,
    pub p_in: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct NearField {
    pub e: CVec,
    pub h: CVec,
}

fn norm(v: &CVec) -> f64 {
    v.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt()
}

impl NearField {
    pub fn e_peak(&self) -> f64 {
        norm(&self.e)
    }

    pub fn h_peak(&self) -> f64 {
        norm(&self.h)
    }
}

fn cvec(v: Vec3, c: C64) -> CVec {
    [c * v[0], c * v[1], c * v[2]]
}

const GL4: [(f64, f64); 4] = [
    (0.069_431_844_202_973_7, 0.173_927_422_568_726_9),
    (0.330_009_478_207_571_9, 0.326_072_577_431_273_1),
    (0.669_990_521_792_428_1, 0.326_072_577_431_273_1),
    (0.930_568_155_797_026_3, 0.173_927_422_568_726_9),
];

impl NearSource {
    pub fn new(model: &Model, coeffs: &[C64], k: f64, p_in: f64) -> Self {
        let n = model.segs.len();
        let mut ia = vec![C64::new(0.0, 0.0); n];
        let mut ib = vec![C64::new(0.0, 0.0); n];
        let mut q = vec![C64::new(0.0, 0.0); n];
        for (b, &c) in model.bases.iter().zip(coeffs) {
            for h in &b.halves {
                let s = &model.segs[h.seg];
                if h.at_b {
                    ib[h.seg] += c * h.sign;
                } else {
                    ia[h.seg] += c * h.sign;
                }
                q[h.seg] += c * (h.sign * if h.at_b { 1.0 } else { -1.0 } / s.len);
            }
        }
        let real = model.real_ground.map(|g| crate::mom::complex_permittivity(g, k));
        let ground = model.ground_z.zip(real);
        let mut elements = Vec::with_capacity(n * (1 + model.images.len()));
        for (i, s) in model.segs.iter().enumerate() {
            let rad = s.rad.unwrap_or(model.a);
            elements.push(Element {
                a: s.a,
                b: s.b,
                len: s.len,
                rad,
                ia: cvec(s.dir, ia[i]),
                ib: cvec(s.dir, ib[i]),
                q: q[i],
                reflected: false,
            });
        }
        for t in &model.images {
            let is_ground = ground.is_some_and(|(z, _)| {
                t.planes.len() == 1 && t.planes[0].axis == 2 && t.planes[0].at == z
            });
            for (i, s) in model.segs.iter().enumerate() {
                elements.push(image(t, s, model.a, ia[i], ib[i], q[i], is_ground));
            }
        }
        NearSource { elements, k, ground, real: n, p_in }
    }

    pub fn at(&self, r: Vec3) -> NearField {
        let k = self.k;
        let zero = C64::new(0.0, 0.0);
        let mut e = [zero; 3];
        let mut h = [zero; 3];
        let jk_eta = C64::new(0.0, k * ETA);
        let j_eta_k = C64::new(0.0, ETA / k);
        for el in &self.elements {
            let mid = lerp(el.a, el.b, 0.5);
            let dist = length(sub(r, mid)).max(el.rad);
            let pieces = ((3.0 * el.len / dist).ceil() as usize).clamp(1, 64);
            let (ia, ib, q) = match (el.reflected, self.ground) {
                (true, Some((gz, eps))) => {
                    let d = sub(r, mid);
                    let _ = gz;
                    let sin_psi = d[2].abs() / length(d).max(1e-12);
                    let (rv, rh) = fresnel(eps, sin_psi.min(1.0));
                    let perp = cross([0.0, 0.0, 1.0], d);
                    let pl = length(perp);
                    let split = |v: CVec| -> CVec {
                        if pl <= 1e-9 * length(d) {
                            return v.map(|c| c * rv);
                        }
                        let u = scale(perp, 1.0 / pl);
                        let across: C64 = (0..3).map(|t| v[t] * u[t]).sum();
                        [0, 1, 2].map(|t| {
                            let ac = across * u[t];
                            (v[t] - ac) * rv - ac * rh
                        })
                    };
                    (split(el.ia), split(el.ib), el.q * rv)
                }
                _ => (el.ia, el.ib, el.q),
            };
            let step = 1.0 / pieces as f64;
            for p in 0..pieces {
                for &(u0, w0) in &GL4 {
                    let u = (p as f64 + u0) * step;
                    let w = w0 * step * el.len;
                    let src = lerp(el.a, el.b, u);
                    let d = sub(r, src);
                    let r2 = dot(d, d) + el.rad * el.rad;
                    let rr = r2.sqrt();
                    let g = C64::new(0.0, -k * rr).exp() / (4.0 * PI * rr);
                    let dg = -g * C64::new(1.0, k * rr) / rr;
                    let rhat = scale(d, 1.0 / rr);
                    let cur: CVec = [0, 1, 2].map(|t| ia[t] * (1.0 - u) + ib[t] * u);
                    for t in 0..3 {
                        e[t] -= (jk_eta * cur[t] * g + j_eta_k * q * dg * rhat[t]) * w;
                    }
                    let c = [
                        rhat[1] * cur[2] - rhat[2] * cur[1],
                        rhat[2] * cur[0] - rhat[0] * cur[2],
                        rhat[0] * cur[1] - rhat[1] * cur[0],
                    ];
                    for t in 0..3 {
                        h[t] += dg * c[t] * w;
                    }
                }
            }
        }
        NearField { e: e.map(|c| c * 1000.0), h: h.map(|c| c * 1000.0) }
    }

    pub fn wires(&self) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        self.elements.iter().take(self.real).map(|e| (e.a, e.b))
    }

    pub fn distance(&self, p: Vec3) -> f64 {
        self.wires()
            .map(|(a, b)| {
                let ab = sub(b, a);
                let t = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-30)).clamp(0.0, 1.0);
                length(sub(p, lerp(a, b, t)))
            })
            .fold(f64::INFINITY, f64::min)
    }

    pub fn scale_for(&self, watts: f64) -> f64 {
        if self.p_in > 0.0 { (watts / self.p_in).sqrt() } else { 0.0 }
    }
}

fn image(
    t: &ImageTransform,
    s: &crate::mom::Segment,
    a_default: f64,
    ia: C64,
    ib: C64,
    q: C64,
    reflected: bool,
) -> Element {
    let (a, b) = (t.apply(s.a), t.apply(s.b));
    let dir = scale(sub(b, a), 1.0 / s.len);
    let sign = if t.reverses_current() { -1.0 } else { 1.0 };
    Element {
        a,
        b,
        len: s.len,
        rad: s.rad.unwrap_or(a_default),
        ia: cvec(dir, ia * sign),
        ib: cvec(dir, ib * sign),
        q: q * sign,
        reflected,
    }
}
