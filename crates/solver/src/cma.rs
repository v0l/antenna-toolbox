use crate::mom::{Model, fill, sommerfeld_correction};
use num_complex::Complex64 as C64;

#[derive(Clone, Debug)]
pub struct Mode {
    pub lambda: f64,
    pub current: Vec<f64>,
    pub weight: C64,
}

impl Mode {
    pub fn significance(&self) -> f64 {
        1.0 / C64::new(1.0, self.lambda).norm()
    }

    pub fn angle_deg(&self) -> f64 {
        180.0 - self.lambda.atan().to_degrees()
    }
}

pub fn impedance(model: &Model, k: f64) -> (Vec<f64>, Vec<f64>, usize) {
    let sys = fill(model, k);
    let (n, w) = (sys.n, sys.w);
    let corr = sommerfeld_correction(model, k);
    let mut r = vec![0.0; n * n];
    let mut x = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut z = sys.a[i * w + j] + sys.a[j * w + i];
            if let Some(c) = &corr {
                z += c[i * n + j] + c[j * n + i];
            }
            r[i * n + j] = z.re / 2.0;
            x[i * n + j] = z.im / 2.0;
        }
    }
    (r, x, n)
}

fn lu(mut a: Vec<f64>, n: usize) -> (Vec<f64>, Vec<usize>) {
    let mut piv: Vec<usize> = (0..n).collect();
    for c in 0..n {
        let p =
            (c..n).max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs())).unwrap_or(c);
        if p != c {
            for k in 0..n {
                a.swap(c * n + k, p * n + k);
            }
            piv.swap(c, p);
        }
        let d = a[c * n + c];
        if d == 0.0 {
            continue;
        }
        for r in c + 1..n {
            let f = a[r * n + c] / d;
            a[r * n + c] = f;
            if f != 0.0 {
                for k in c + 1..n {
                    a[r * n + k] -= f * a[c * n + k];
                }
            }
        }
    }
    (a, piv)
}

fn lu_solve(f: &(Vec<f64>, Vec<usize>), n: usize, b: &[f64]) -> Vec<f64> {
    let (a, piv) = f;
    let mut x: Vec<f64> = piv.iter().map(|&p| b[p]).collect();
    for r in 0..n {
        let s: f64 = (0..r).map(|k| a[r * n + k] * x[k]).sum();
        x[r] -= s;
    }
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r * n + k] * x[k]).sum();
        let d = a[r * n + r];
        x[r] = (x[r] - s) / if d == 0.0 { 1e-300 } else { d };
    }
    x
}

fn matvec(m: &[f64], n: usize, v: &[f64]) -> Vec<f64> {
    (0..n).map(|i| (0..n).map(|j| m[i * n + j] * v[j]).sum()).collect()
}

pub fn modes(model: &Model, k: f64, count: usize) -> Vec<Mode> {
    let (r, x, n) = impedance(model, k);
    if n == 0 {
        return Vec::new();
    }
    let p = (count + 6).min(n);
    let sigma = 0.0137;
    let shifted: Vec<f64> = x.iter().zip(&r).map(|(x, r)| x - sigma * r).collect();
    let fact = lu(shifted, n);
    let mut vs: Vec<Vec<f64>> = (0..p)
        .map(|c| {
            (0..n)
                .map(|i| (((i * 31 + c * 17) % 13) as f64 - 6.0) + if i == c { 5.0 } else { 0.0 })
                .collect()
        })
        .collect();
    let mut last: Vec<f64> = Vec::new();
    let mut ritz: Vec<Mode> = Vec::new();
    for _ in 0..80 {
        for v in vs.iter_mut() {
            *v = lu_solve(&fact, n, &matvec(&r, n, v));
        }
        r_orthonormalise(&mut vs, &r, n);
        ritz = vs
            .iter()
            .filter(|v| {
                let rv = matvec(&r, n, v);
                let q: f64 = v.iter().zip(&rv).map(|(a, b)| a * b).sum();
                (q - 1.0).abs() < 1e-3
            })
            .map(|v| {
                let xv = matvec(&x, n, v);
                let lambda = v.iter().zip(&xv).map(|(a, b)| a * b).sum();
                let mut current = v.clone();
                let big = current
                    .iter()
                    .copied()
                    .fold(0.0f64, |a, b| if b.abs() > a.abs() { b } else { a });
                if big < 0.0 {
                    for c in current.iter_mut() {
                        *c = -*c;
                    }
                }
                Mode { lambda, current, weight: C64::new(0.0, 0.0) }
            })
            .collect();
        let lams: Vec<f64> = ritz.iter().take(count.min(p)).map(|m| m.lambda).collect();
        let done = last.len() == lams.len()
            && lams.iter().zip(&last).all(|(a, b)| (a - b).abs() <= 1e-10 * (1.0 + a.abs()));
        last = lams;
        if done {
            break;
        }
    }
    ritz.sort_by(|a, b| a.lambda.abs().total_cmp(&b.lambda.abs()));
    let mut feed = vec![0.0; n];
    if model.feed < n {
        feed[model.feed] = 1.0;
    }
    let src: Vec<(usize, C64)> = model.sources.clone();
    for m in ritz.iter_mut() {
        let mut v: C64 = C64::new(m.current.iter().zip(&feed).map(|(j, e)| j * e).sum(), 0.0);
        for &(b, s) in &src {
            if b < n {
                v += s * m.current[b];
            }
        }
        m.weight = v / C64::new(1.0, m.lambda);
    }
    ritz.truncate(count.min(p));
    ritz
}

fn r_orthonormalise(vs: &mut [Vec<f64>], r: &[f64], n: usize) {
    for i in 0..vs.len() {
        for j in 0..i {
            let rj = matvec(r, n, &vs[j]);
            let d: f64 = vs[i].iter().zip(&rj).map(|(a, b)| a * b).sum();
            let vj = vs[j].clone();
            for (a, b) in vs[i].iter_mut().zip(&vj) {
                *a -= d * b;
            }
        }
        let ri = matvec(r, n, &vs[i]);
        let norm = vs[i].iter().zip(&ri).map(|(a, b)| a * b).sum::<f64>().max(0.0).sqrt();
        if norm > 0.0 {
            for a in vs[i].iter_mut() {
                *a /= norm;
            }
        }
    }
}

pub fn overlap(r: &[f64], n: usize, a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..n {
        let rb: f64 = (0..n).map(|j| r[i * n + j] * b[j]).sum();
        s += a[i] * rb;
    }
    s.abs()
}
