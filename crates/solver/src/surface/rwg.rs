use super::mesh::{RwgEdge, SurfaceTopology, Triangle};
use crate::linalg::System;
use crate::mom::{FieldFn, transverse};
use crate::units::ETA;
use crate::vec::{Vec3, add, dot, length, scale, sub};
use num_complex::Complex64 as C64;
use rayon::prelude::*;
use std::f64::consts::PI;
use std::sync::Arc;

const J: C64 = C64 { re: 0.0, im: 1.0 };

const QUAD: [(f64, [f64; 3]); 7] = [
    (0.225, [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]),
    (0.132394152788506, [0.059715871789770, 0.470142064105115, 0.470142064105115]),
    (0.132394152788506, [0.470142064105115, 0.059715871789770, 0.470142064105115]),
    (0.132394152788506, [0.470142064105115, 0.470142064105115, 0.059715871789770]),
    (0.125939180544827, [0.797426985353087, 0.101286507323456, 0.101286507323456]),
    (0.125939180544827, [0.101286507323456, 0.797426985353087, 0.101286507323456]),
    (0.125939180544827, [0.101286507323456, 0.101286507323456, 0.797426985353087]),
];

struct Analytic {
    scalar: f64,
    vector: Vec3,
}

fn analytic_integrals(tri: &Triangle, verts: &[Vec3], r: Vec3) -> Analytic {
    let n = tri.normal;
    let p0 = verts[tri.v[0]];
    let h = dot(sub(r, p0), n);
    let proj = sub(r, scale(n, h));
    let abs_h = h.abs();

    let mut scalar_sum = 0.0;
    let mut beta_sum = 0.0;
    let mut vec = [0.0; 3];

    for i in 0..3 {
        let a = verts[tri.v[i]];
        let b = verts[tri.v[(i + 1) % 3]];
        let edge = sub(b, a);
        let len = length(edge);
        if len < 1e-12 {
            continue;
        }
        let l_hat = scale(edge, 1.0 / len);
        let u_hat = [
            l_hat[1] * n[2] - l_hat[2] * n[1],
            l_hat[2] * n[0] - l_hat[0] * n[2],
            l_hat[0] * n[1] - l_hat[1] * n[0],
        ];
        let p0v = dot(sub(a, proj), u_hat);
        let l_minus = dot(sub(a, proj), l_hat);
        let l_plus = dot(sub(b, proj), l_hat);
        let r_minus = length(sub(a, r));
        let r_plus = length(sub(b, r));
        let r0sq = p0v * p0v + h * h;

        let dm = r_minus + l_minus;
        let dp = r_plus + l_plus;
        let f2 = if dm < 1e-5 * len || dp < 1e-5 * len { 0.0 } else { (dp / dm).ln() };

        scalar_sum += p0v * f2;
        vec = add(vec, scale(u_hat, 0.5 * (r0sq * f2 + l_plus * r_plus - l_minus * r_minus)));

        if abs_h > 1e-12 {
            let t1 = (p0v * l_plus).atan2(r0sq + abs_h * r_plus);
            let t2 = (p0v * l_minus).atan2(r0sq + abs_h * r_minus);
            beta_sum += t1 - t2;
        }
    }
    Analytic { scalar: scalar_sum - abs_h * beta_sum, vector: vec }
}

struct SourceIntegrals {
    scalar: C64,
    vector: [C64; 3],
}

fn source_integrals(
    tri: &Triangle,
    verts: &[Vec3],
    free: Vec3,
    r: Vec3,
    k: f64,
) -> SourceIntegrals {
    let (a, b, c) = (verts[tri.v[0]], verts[tri.v[1]], verts[tri.v[2]]);
    let mut sr = C64::new(0.0, 0.0);
    let mut vr = [C64::new(0.0, 0.0); 3];
    for (w, l) in QUAD {
        let p = add(add(scale(a, l[0]), scale(b, l[1])), scale(c, l[2]));
        let rr = length(sub(p, r)).max(1e-12);
        let kr = k * rr;
        let g = C64::new((kr.cos() - 1.0) / rr, -kr.sin() / rr);
        let w = w * tri.area / (4.0 * PI);
        sr += g * w;
        let rho = sub(p, free);
        for t in 0..3 {
            vr[t] += g * (w * rho[t]);
        }
    }
    let an = analytic_integrals(tri, verts, r);
    let n = tri.normal;
    let h = dot(sub(r, a), n);
    let proj = sub(r, scale(n, h));
    let offset = sub(proj, free);
    let inv = 1.0 / (4.0 * PI);
    let scalar = C64::new(sr.re + inv * an.scalar, sr.im);
    let mut vector = [C64::new(0.0, 0.0); 3];
    for t in 0..3 {
        vector[t] = C64::new(vr[t].re + inv * (an.vector[t] + offset[t] * an.scalar), vr[t].im);
    }
    SourceIntegrals { scalar, vector }
}

#[derive(Clone, Copy)]
pub struct EdgeGeom {
    pub centre_plus: Vec3,
    pub centre_minus: Vec3,
    pub rho_plus: Vec3,
    pub rho_minus: Vec3,
}

pub fn edge_geometry(e: &RwgEdge, topo: &SurfaceTopology) -> EdgeGeom {
    let tp = &topo.triangles[e.plus];
    let tm = &topo.triangles[e.minus];
    EdgeGeom {
        centre_plus: tp.centre,
        centre_minus: tm.centre,
        rho_plus: sub(tp.centre, topo.mesh.vertices[e.free_plus]),
        rho_minus: sub(topo.mesh.vertices[e.free_minus], tm.centre),
    }
}

pub fn fill_surface(topo: &SurfaceTopology, k: f64, feed: &[usize]) -> System {
    let edges = &topo.edges;
    let n = edges.len();
    let mut sys = System::zeros(n);
    let w = sys.w;
    let verts = &topo.mesh.vertices;
    let geom: Vec<EdgeGeom> = edges.iter().map(|e| edge_geometry(e, topo)).collect();
    let omega_mu = k * ETA;
    let inv_omega_eps = ETA / k;

    sys.a.par_chunks_mut(w).enumerate().for_each(|(m, row)| {
        let em = &edges[m];
        let gm = &geom[m];
        for (ni, en) in edges.iter().enumerate() {
            let tri_plus = &topo.triangles[en.plus];
            let tri_minus = &topo.triangles[en.minus];
            let free_plus = verts[en.free_plus];
            let free_minus = verts[en.free_minus];
            let mut z = C64::new(0.0, 0.0);
            for side in [1.0f64, -1.0] {
                let (obs, rho) = if side > 0.0 {
                    (gm.centre_plus, gm.rho_plus)
                } else {
                    (gm.centre_minus, gm.rho_minus)
                };
                let ip = source_integrals(tri_plus, verts, free_plus, obs, k);
                let im = source_integrals(tri_minus, verts, free_minus, obs, k);
                let mut a_dot_rho = C64::new(0.0, 0.0);
                for t in 0..3 {
                    a_dot_rho += ip.vector[t] * (en.length / (2.0 * tri_plus.area) * rho[t]);
                    a_dot_rho -= im.vector[t] * (en.length / (2.0 * tri_minus.area) * rho[t]);
                }
                let vec_part = J * a_dot_rho * (omega_mu / 2.0);
                let phi = ip.scalar * (en.length / tri_plus.area)
                    - im.scalar * (en.length / tri_minus.area);
                let scalar_part = J * phi * inv_omega_eps * if side > 0.0 { -1.0 } else { 1.0 };
                z += vec_part + scalar_part;
            }
            row[ni] = z * em.length;
        }
    });
    for &f in feed {
        *sys.rhs_mut(f) = C64::new(edges[f].length, 0.0);
    }
    sys
}

pub fn surface_impedance(edges: &[RwgEdge], feed: &[usize], x: &[C64]) -> C64 {
    let i: C64 = feed.iter().map(|&f| x[f] * edges[f].length).sum();
    if i.norm_sqr() > 0.0 { i.inv() } else { C64::new(1e30, 0.0) }
}

pub fn surface_field(topo: &SurfaceTopology, x: Arc<Vec<C64>>, k: f64) -> FieldFn {
    let geom: Vec<(EdgeGeom, f64)> =
        topo.edges.iter().map(|e| (edge_geometry(e, topo), e.length / 2.0)).collect();
    Arc::new(move |dir| {
        let mut fr = [0.0; 3];
        let mut fi = [0.0; 3];
        for (n, (g, half)) in geom.iter().enumerate() {
            for (centre, rho) in [(g.centre_plus, g.rho_plus), (g.centre_minus, g.rho_minus)] {
                let ph = k * dot(dir, centre);
                let wv = x[n] * C64::new(ph.cos(), ph.sin()) * *half;
                for t in 0..3 {
                    fr[t] += wv.re * rho[t];
                    fi[t] += wv.im * rho[t];
                }
            }
        }
        transverse(fr, fi, dir)
    })
}
