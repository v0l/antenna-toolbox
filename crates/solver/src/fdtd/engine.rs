use super::mesh::Grid;
use super::{C0, EPS0, MU0, Model};
use rayon::prelude::*;

#[derive(Clone, Copy)]
struct Shared<T>(*mut T);
unsafe impl<T> Send for Shared<T> {}
unsafe impl<T> Sync for Shared<T> {}

impl<T> Shared<T> {
    fn get(self) -> *mut T {
        self.0
    }
}

#[derive(Clone)]
pub struct Axis {
    pub n: usize,
    pub d: Vec<f64>,
    pub dd: Vec<f64>,
    pub inv_d: Vec<f32>,
    pub inv_dd: Vec<f32>,
    pub raw_inv_d: Vec<f32>,
    pub raw_inv_dd: Vec<f32>,
    pub be: Vec<f32>,
    pub ce: Vec<f32>,
    pub bh: Vec<f32>,
    pub ch: Vec<f32>,
    pub slot_e: Vec<i32>,
    pub slot_h: Vec<i32>,
}

fn axis(line: &[f64], pml: usize, dt: f64) -> Axis {
    let n = line.len();
    let d: Vec<f64> = (0..n)
        .map(|i| if i + 1 < n { line[i + 1] - line[i] } else { line[i] - line[i - 1] })
        .collect();
    let dd: Vec<f64> = (0..n)
        .map(|i| {
            if i == 0 {
                d[0]
            } else if i + 1 >= n {
                d[n - 2]
            } else {
                0.5 * (line[i + 1] - line[i - 1])
            }
        })
        .collect();
    let (m, kappa_max, alpha_max) = (3.0, 1.0, 0.05);
    let mut ke = vec![1.0; n];
    let mut kh = vec![1.0; n];
    let mut be = vec![0f32; n];
    let mut ce = vec![0f32; n];
    let mut bh = vec![0f32; n];
    let mut ch = vec![0f32; n];
    let mut slot_e = vec![-1i32; n];
    let mut slot_h = vec![-1i32; n];
    if pml > 0 && n > 2 * pml + 2 {
        let left = line[pml];
        let right = line[n - 1 - pml];
        let depth_l = left - line[0];
        let depth_r = line[n - 1] - right;
        let coef = |rho: f64, depth: f64, cell: f64| {
            let r = (rho / depth).clamp(0.0, 1.0);
            let sigma_max = 0.8 * (m + 1.0) / (super::ETA0 * cell);
            let sigma = sigma_max * r.powf(m);
            let kappa = 1.0 + (kappa_max - 1.0) * r.powf(m);
            let alpha = alpha_max * (1.0 - r) * 2.0 * std::f64::consts::PI * EPS0 * 1e9;
            let b = (-(sigma / kappa + alpha) * dt / EPS0).exp();
            let c = if sigma > 0.0 {
                sigma * (b - 1.0) / (sigma * kappa + kappa * kappa * alpha)
            } else {
                0.0
            };
            (kappa, b, c)
        };
        for i in 0..n {
            let x = line[i];
            let (rho, depth, cell) = if x < left {
                (left - x, depth_l, d[0])
            } else if x > right {
                (x - right, depth_r, d[n - 2])
            } else {
                (0.0, 1.0, 1.0)
            };
            if rho > 0.0 {
                let (k, b, c) = coef(rho, depth, cell);
                ke[i] = k;
                be[i] = b as f32;
                ce[i] = c as f32;
            }
            if i < pml {
                slot_e[i] = i as i32;
            } else if i > n - 1 - pml {
                slot_e[i] = (i + 2 * pml - n) as i32;
            }
            if i + 1 < n {
                let xc = 0.5 * (line[i] + line[i + 1]);
                let (rho, depth, cell) = if xc < left {
                    (left - xc, depth_l, d[0])
                } else if xc > right {
                    (xc - right, depth_r, d[n - 2])
                } else {
                    (0.0, 1.0, 1.0)
                };
                if rho > 0.0 {
                    let (k, b, c) = coef(rho, depth, cell);
                    kh[i] = k;
                    bh[i] = b as f32;
                    ch[i] = c as f32;
                }
                if i < pml {
                    slot_h[i] = i as i32;
                } else if i >= n - 1 - pml {
                    slot_h[i] = (pml + i - (n - 1 - pml)) as i32;
                }
            }
        }
    }
    Axis {
        n,
        inv_d: (0..n).map(|i| (1.0 / (kh[i] * d[i])) as f32).collect(),
        inv_dd: (0..n).map(|i| (1.0 / (ke[i] * dd[i])) as f32).collect(),
        raw_inv_d: d.iter().map(|v| (1.0 / v) as f32).collect(),
        raw_inv_dd: dd.iter().map(|v| (1.0 / v) as f32).collect(),
        d,
        dd,
        be,
        ce,
        bh,
        ch,
        slot_e,
        slot_h,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PortEdges {
    pub axis: usize,
    pub at: [usize; 3],
    pub count: usize,
    pub r: f64,
}

pub struct Sim {
    pub grid: Grid,
    pub ax: [Axis; 3],
    pub dt: f64,
    pub e: [Vec<f32>; 3],
    pub h: [Vec<f32>; 3],
    pub ca: [Vec<f32>; 3],
    pub cb: [Vec<f32>; 3],
    pub psi_e: [[Vec<f32>; 3]; 3],
    pub psi_h: [[Vec<f32>; 3]; 3],
    pub port: Option<PortEdges>,
    pub port_coef: Vec<(usize, f32, f32, f32)>,
    pub pml: usize,
    pub steps: usize,
}

pub fn idx(n: [usize; 3], i: usize, j: usize, k: usize) -> usize {
    (i * n[1] + j) * n[2] + k
}

fn psi_len(n: [usize; 3], axis: usize, pml: usize) -> usize {
    let mut m = n;
    m[axis] = 2 * pml;
    if pml == 0 { 0 } else { m[0] * m[1] * m[2] }
}

fn psi_idx(
    n: [usize; 3],
    axis: usize,
    pml: usize,
    i: usize,
    j: usize,
    k: usize,
    slot: usize,
) -> usize {
    match axis {
        0 => (slot * n[1] + j) * n[2] + k,
        1 => (i * 2 * pml + slot) * n[2] + k,
        _ => (i * n[1] + j) * 2 * pml + slot,
    }
}

pub struct Materials {
    pub eps: Vec<f64>,
    pub sigma: Vec<f64>,
}

impl Materials {
    pub fn from_model(grid: &Grid, model: &Model) -> Materials {
        let [nx, ny, nz] = grid.dims();
        let cells = (nx - 1) * (ny - 1) * (nz - 1);
        let mut eps = vec![1.0; cells];
        let mut sigma = vec![0.0; cells];
        let w = 2.0 * std::f64::consts::PI * model.f0;
        for d in &model.dielectrics {
            let range = |a: usize| {
                let ax = grid.axis(a);
                let lo = ax.partition_point(|v| *v < d.bounds.lo[a] - 1e-9);
                let hi = ax.partition_point(|v| *v <= d.bounds.hi[a] + 1e-9);
                lo..hi.saturating_sub(1).max(lo)
            };
            for i in range(0) {
                for j in range(1) {
                    for k in range(2) {
                        let c = (i * (ny - 1) + j) * (nz - 1) + k;
                        eps[c] = d.eps_r;
                        sigma[c] = w * EPS0 * d.eps_r * d.tan_d;
                    }
                }
            }
        }
        Materials { eps, sigma }
    }

    fn at(&self, dims: [usize; 3], i: isize, j: isize, k: isize) -> (f64, f64) {
        let c = [
            i.clamp(0, dims[0] as isize - 2),
            j.clamp(0, dims[1] as isize - 2),
            k.clamp(0, dims[2] as isize - 2),
        ];
        let id = (c[0] as usize * (dims[1] - 1) + c[1] as usize) * (dims[2] - 1) + c[2] as usize;
        (self.eps[id], self.sigma[id])
    }
}

pub fn time_step(grid: &Grid) -> f64 {
    let min = |v: &[f64]| v.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min);
    let s = 1.0 / min(&grid.x).powi(2) + 1.0 / min(&grid.y).powi(2) + 1.0 / min(&grid.z).powi(2);
    0.99 / (C0 * s.sqrt())
}

impl Sim {
    pub fn new(
        grid: Grid,
        mats: &Materials,
        metal: &[super::Aabb],
        port: Option<super::Port>,
    ) -> Sim {
        let dt = time_step(&grid);
        let pml = grid.pml;
        let ax = [axis(&grid.x, pml, dt), axis(&grid.y, pml, dt), axis(&grid.z, pml, dt)];
        let n = grid.dims();
        let total = n[0] * n[1] * n[2];
        let mut ca = [vec![0f32; total], vec![0f32; total], vec![0f32; total]];
        let mut cb = [vec![0f32; total], vec![0f32; total], vec![0f32; total]];
        for c in 0..3 {
            let (u, v) = ((c + 1) % 3, (c + 2) % 3);
            let du = &ax[u].d;
            let dv = &ax[v].d;
            let (cac, cbc) = (&mut ca[c], &mut cb[c]);
            for i in 0..n[0] {
                for j in 0..n[1] {
                    for k in 0..n[2] {
                        let p = [i, j, k];
                        if p[c] + 1 >= n[c]
                            || p[u] == 0
                            || p[u] + 1 >= n[u]
                            || p[v] == 0
                            || p[v] + 1 >= n[v]
                        {
                            continue;
                        }
                        let mut e_sum = 0.0;
                        let mut s_sum = 0.0;
                        let mut w_sum = 0.0;
                        for (ou, ov) in [(-1isize, -1isize), (0, -1), (-1, 0), (0, 0)] {
                            let mut cell = [p[0] as isize, p[1] as isize, p[2] as isize];
                            cell[u] += ou;
                            cell[v] += ov;
                            let w = du[cell[u] as usize] * dv[cell[v] as usize];
                            let (e, s) = mats.at(n, cell[0], cell[1], cell[2]);
                            e_sum += e * w;
                            s_sum += s * w;
                            w_sum += w;
                        }
                        let eps = EPS0 * e_sum / w_sum;
                        let sig = s_sum / w_sum;
                        let x = sig * dt / (2.0 * eps);
                        let id = idx(n, i, j, k);
                        cac[id] = ((1.0 - x) / (1.0 + x)) as f32;
                        cbc[id] = (dt / eps / (1.0 + x)) as f32;
                    }
                }
            }
        }
        for m in metal {
            let range = |a: usize| {
                let axv = grid.axis(a);
                let lo = axv.partition_point(|v| *v < m.lo[a] - 1e-9);
                let hi = axv.partition_point(|v| *v <= m.hi[a] + 1e-9);
                (lo, hi)
            };
            let r = [range(0), range(1), range(2)];
            for c in 0..3 {
                let mut lim = r;
                lim[c].1 = lim[c].1.saturating_sub(1);
                for i in lim[0].0..lim[0].1 {
                    for j in lim[1].0..lim[1].1 {
                        for k in lim[2].0..lim[2].1 {
                            let id = idx(n, i, j, k);
                            ca[c][id] = 0.0;
                            cb[c][id] = 0.0;
                        }
                    }
                }
            }
        }
        let psi = |c: usize| {
            [0, 1, 2].map(|a| if a == c { Vec::new() } else { vec![0f32; psi_len(n, a, pml)] })
        };
        let mut sim = Sim {
            e: [vec![0f32; total], vec![0f32; total], vec![0f32; total]],
            h: [vec![0f32; total], vec![0f32; total], vec![0f32; total]],
            ca,
            cb,
            psi_e: [psi(0), psi(1), psi(2)],
            psi_h: [psi(0), psi(1), psi(2)],
            port: None,
            port_coef: Vec::new(),
            ax,
            grid,
            dt,
            pml,
            steps: 0,
        };
        if let Some(p) = port {
            sim.set_port(p);
        }
        sim
    }

    fn set_port(&mut self, p: super::Port) {
        let axis = (0..3)
            .max_by(|a, b| (p.b[*a] - p.a[*a]).abs().total_cmp(&(p.b[*b] - p.a[*b]).abs()))
            .unwrap_or(2);
        let at = [0, 1, 2].map(|t| self.grid.nearest(t, p.a[t].min(p.b[t])));
        let end = self.grid.nearest(axis, p.a[axis].max(p.b[axis]));
        let count = end.saturating_sub(at[axis]).max(1);
        let n = self.grid.dims();
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let re = p.r / count as f64;
        let mut coef = Vec::new();
        for s in 0..count {
            let mut q = at;
            q[axis] += s;
            let id = idx(n, q[0], q[1], q[2]);
            let area = self.ax[u].dd[q[u]] * self.ax[v].dd[q[v]];
            let dl = self.ax[axis].d[q[axis]];
            let cb = self.cb[axis][id] as f64;
            let ca = self.ca[axis][id] as f64;
            let x_sig = (1.0 - ca) / (1.0 + ca);
            let eps_dt = if cb > 0.0 { self.dt / (cb * (1.0 + x_sig)) } else { EPS0 * self.dt };
            let eps = eps_dt / self.dt;
            let beta = self.dt * dl / (2.0 * re * eps * area) + x_sig;
            let new_ca = (1.0 - beta) / (1.0 + beta);
            let new_cb = self.dt / (eps * (1.0 + beta));
            let src = self.dt / (eps * (1.0 + beta) * re * area);
            self.ca[axis][id] = new_ca as f32;
            self.cb[axis][id] = new_cb as f32;
            coef.push((id, src as f32, dl as f32, 0.0));
        }
        self.port = Some(PortEdges { axis, at, count, r: p.r });
        self.port_coef = coef;
    }

    pub fn dims(&self) -> [usize; 3] {
        self.grid.dims()
    }

    pub fn update_h(&mut self) {
        let n = self.dims();
        let pml = self.pml;
        let k_mu = (self.dt / MU0) as f32;
        let ax = &self.ax;
        let e = [
            Shared(self.e[0].as_mut_ptr()),
            Shared(self.e[1].as_mut_ptr()),
            Shared(self.e[2].as_mut_ptr()),
        ];
        let h = [
            Shared(self.h[0].as_mut_ptr()),
            Shared(self.h[1].as_mut_ptr()),
            Shared(self.h[2].as_mut_ptr()),
        ];
        let psi = [0, 1, 2].map(|c| [0, 1, 2].map(|a| Shared(self.psi_h[c][a].as_mut_ptr())));
        (0..n[0]).into_par_iter().for_each(|i| unsafe {
            for c in 0..3 {
                let (u, v) = ((c + 1) % 3, (c + 2) % 3);
                let (eu, ev) = (e[u].get(), e[v].get());
                let hc = h[c].get();
                for j in 0..n[1] {
                    for k in 0..n[2] {
                        let p = [i, j, k];
                        if p[u] + 1 >= n[u] || p[v] + 1 >= n[v] || (c != 0 && i + 1 >= n[0]) {
                            continue;
                        }
                        if c == 0 && i >= n[0] {
                            continue;
                        }
                        let id = idx(n, i, j, k);
                        let mut pu = p;
                        pu[u] += 1;
                        let mut pv = p;
                        pv[v] += 1;
                        let dv_du = *ev.add(idx(n, pu[0], pu[1], pu[2])) - *ev.add(id);
                        let du_dv = *eu.add(idx(n, pv[0], pv[1], pv[2])) - *eu.add(id);
                        let mut t1 = dv_du * ax[u].inv_d[p[u]];
                        let mut t2 = du_dv * ax[v].inv_d[p[v]];
                        if pml > 0 {
                            let su = ax[u].slot_h[p[u]];
                            if su >= 0 {
                                let q =
                                    psi[c][u].get().add(psi_idx(n, u, pml, i, j, k, su as usize));
                                *q = ax[u].bh[p[u]] * *q
                                    + ax[u].ch[p[u]] * dv_du * ax[u].raw_inv_d[p[u]];
                                t1 += *q;
                            }
                            let sv = ax[v].slot_h[p[v]];
                            if sv >= 0 {
                                let q =
                                    psi[c][v].get().add(psi_idx(n, v, pml, i, j, k, sv as usize));
                                *q = ax[v].bh[p[v]] * *q
                                    + ax[v].ch[p[v]] * du_dv * ax[v].raw_inv_d[p[v]];
                                t2 += *q;
                            }
                        }
                        *hc.add(id) -= k_mu * (t1 - t2);
                    }
                }
            }
        });
    }

    pub fn update_e(&mut self, vs: f32) {
        let n = self.dims();
        let pml = self.pml;
        let ax = &self.ax;
        let e = [
            Shared(self.e[0].as_mut_ptr()),
            Shared(self.e[1].as_mut_ptr()),
            Shared(self.e[2].as_mut_ptr()),
        ];
        let h = [
            Shared(self.h[0].as_mut_ptr()),
            Shared(self.h[1].as_mut_ptr()),
            Shared(self.h[2].as_mut_ptr()),
        ];
        let psi = [0, 1, 2].map(|c| [0, 1, 2].map(|a| Shared(self.psi_e[c][a].as_mut_ptr())));
        let (ca, cb) = (&self.ca, &self.cb);
        (0..n[0]).into_par_iter().for_each(|i| unsafe {
            for c in 0..3 {
                let (u, v) = ((c + 1) % 3, (c + 2) % 3);
                let (hu, hv) = (h[u].get(), h[v].get());
                let ec = e[c].get();
                for j in 0..n[1] {
                    for k in 0..n[2] {
                        let p = [i, j, k];
                        let id = idx(n, i, j, k);
                        let cbv = cb[c][id];
                        if cbv == 0.0 {
                            continue;
                        }
                        let mut pu = p;
                        pu[u] -= 1;
                        let mut pv = p;
                        pv[v] -= 1;
                        let dv_du = *hv.add(id) - *hv.add(idx(n, pu[0], pu[1], pu[2]));
                        let du_dv = *hu.add(id) - *hu.add(idx(n, pv[0], pv[1], pv[2]));
                        let mut t1 = dv_du * ax[u].inv_dd[p[u]];
                        let mut t2 = du_dv * ax[v].inv_dd[p[v]];
                        if pml > 0 {
                            let su = ax[u].slot_e[p[u]];
                            if su >= 0 {
                                let q =
                                    psi[c][u].get().add(psi_idx(n, u, pml, i, j, k, su as usize));
                                *q = ax[u].be[p[u]] * *q
                                    + ax[u].ce[p[u]] * dv_du * ax[u].raw_inv_dd[p[u]];
                                t1 += *q;
                            }
                            let sv = ax[v].slot_e[p[v]];
                            if sv >= 0 {
                                let q =
                                    psi[c][v].get().add(psi_idx(n, v, pml, i, j, k, sv as usize));
                                *q = ax[v].be[p[v]] * *q
                                    + ax[v].ce[p[v]] * du_dv * ax[v].raw_inv_dd[p[v]];
                                t2 += *q;
                            }
                        }
                        *ec.add(id) = ca[c][id] * *ec.add(id) + cbv * (t1 - t2);
                    }
                }
            }
        });
        if let Some(p) = self.port {
            let per = vs / p.count as f32;
            for &(id, src, _, _) in &self.port_coef {
                self.e[p.axis][id] -= src * per;
            }
        }
        self.steps += 1;
    }

    pub fn port_voltage(&self) -> f64 {
        let Some(p) = self.port else {
            return 0.0;
        };
        -self.port_coef.iter().map(|&(id, _, dl, _)| (self.e[p.axis][id] * dl) as f64).sum::<f64>()
    }

    pub fn current_probe(&self) -> Option<([usize; 3], f64, f64, usize, usize)> {
        let p = self.port?;
        let n = self.dims();
        let (u, v) = ((p.axis + 1) % 3, (p.axis + 2) % 3);
        let mut q = p.at;
        q[p.axis] += p.count / 2;
        let id = idx(n, q[0], q[1], q[2]);
        let mut qu = q;
        qu[u] -= 1;
        let mut qv = q;
        qv[v] -= 1;
        Some((
            [id, idx(n, qu[0], qu[1], qu[2]), idx(n, qv[0], qv[1], qv[2])],
            self.ax[v].dd[q[v]],
            self.ax[u].dd[q[u]],
            u,
            v,
        ))
    }

    pub fn port_current(&self) -> f64 {
        let Some(p) = self.port else {
            return 0.0;
        };
        let n = self.dims();
        let (u, v) = ((p.axis + 1) % 3, (p.axis + 2) % 3);
        let mut q = p.at;
        q[p.axis] += p.count / 2;
        let id = idx(n, q[0], q[1], q[2]);
        let mut qu = q;
        qu[u] -= 1;
        let mut qv = q;
        qv[v] -= 1;
        let hv = &self.h[v];
        let hu = &self.h[u];
        ((hv[id] - hv[idx(n, qu[0], qu[1], qu[2])]) as f64) * self.ax[v].dd[q[v]]
            - ((hu[id] - hu[idx(n, qv[0], qv[1], qv[2])]) as f64) * self.ax[u].dd[q[u]]
    }

    pub fn energy(&self) -> f64 {
        self.e.iter().map(|c| c.par_iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>()).sum()
    }
}

pub fn pulse_shape(fc: f64) -> (f64, f64) {
    let tau = 10f64.ln().sqrt() / (std::f64::consts::PI * fc);
    (4.0 * tau, tau)
}

pub fn gaussian(f0: f64, fc: f64) -> impl Fn(f64) -> f64 {
    let (t0, tau) = pulse_shape(fc);
    move |t: f64| {
        let x = (t - t0) / tau;
        (-x * x).exp() * (2.0 * std::f64::consts::PI * f0 * (t - t0)).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_cavity_rings_at_its_analytic_modes() {
        let (a, b, d) = (0.10, 0.07, 0.05);
        let mut xs: Vec<f64> = Vec::new();
        let mut t = 0.0;
        let mut s = 0.0035;
        while t < a - 1e-9 {
            xs.push(t);
            t += s;
            s = (s * 1.06).min(0.0055);
        }
        xs.push(a);
        let ys: Vec<f64> = (0..=18).map(|i| b * i as f64 / 18.0).collect();
        let zs: Vec<f64> = (0..=13).map(|i| d * i as f64 / 13.0).collect();
        let grid = Grid { x: xs, y: ys, z: zs, pml: 0 };
        let n = grid.dims();
        let cells = (n[0] - 1) * (n[1] - 1) * (n[2] - 1);
        let mats = Materials { eps: vec![1.0; cells], sigma: vec![0.0; cells] };
        let mut sim = Sim::new(grid, &mats, &[], None);
        let src = idx(n, n[0] / 3, n[1] / 3, n[2] / 3);
        let probe = idx(n, 2 * n[0] / 3, n[1] / 2 + 1, n[2] / 4);
        let pulse = gaussian(3.0e9, 2.5e9);
        let steps = 40_000;
        let mut rec = Vec::with_capacity(steps);
        for step in 0..steps {
            sim.update_h();
            sim.update_e(0.0);
            let t = step as f64 * sim.dt;
            for c in 0..3 {
                sim.e[c][src] += pulse(t) as f32;
            }
            rec.push(sim.e[1][probe] as f64 + sim.e[2][probe] as f64 + sim.e[0][probe] as f64);
        }
        let spectrum = |f: f64| {
            let w = 2.0 * std::f64::consts::PI * f * sim.dt;
            let (mut re, mut im) = (0.0, 0.0);
            for (s, v) in rec.iter().enumerate() {
                re += v * (w * s as f64).cos();
                im -= v * (w * s as f64).sin();
            }
            (re * re + im * im).sqrt()
        };
        let modes = [(1, 1, 0), (1, 0, 1), (0, 1, 1), (2, 1, 0), (1, 1, 1)];
        for (m, nn, p) in modes {
            let fa = C0 / 2.0
                * ((m as f64 / a).powi(2) + (nn as f64 / b).powi(2) + (p as f64 / d).powi(2))
                    .sqrt();
            let peak = (0..=400)
                .map(|s| fa * (0.97 + 0.06 * s as f64 / 400.0))
                .max_by(|x, y| spectrum(*x).total_cmp(&spectrum(*y)))
                .unwrap();
            let err = (peak / fa - 1.0).abs();
            assert!(err < 0.004, "mode {m}{nn}{p}: {peak:.4e} vs {fa:.4e} ({:.3}%)", err * 100.0);
        }
    }
}
