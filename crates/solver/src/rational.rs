use num_complex::Complex64 as C64;

#[derive(Clone, Debug)]
pub struct Rational {
    support: Vec<(f64, C64)>,
    weights: Vec<C64>,
}

fn solve_small(mut a: Vec<C64>, mut b: Vec<C64>, n: usize) -> Option<Vec<C64>> {
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i * n + c].norm().total_cmp(&a[j * n + c].norm()))?;
        if a[p * n + c].norm() == 0.0 {
            return None;
        }
        if p != c {
            for k in 0..n {
                a.swap(c * n + k, p * n + k);
            }
            b.swap(c, p);
        }
        let d = a[c * n + c];
        for r in c + 1..n {
            let f = a[r * n + c] / d;
            if f == C64::new(0.0, 0.0) {
                continue;
            }
            for k in c..n {
                let v = a[c * n + k];
                a[r * n + k] -= f * v;
            }
            let v = b[c];
            b[r] -= f * v;
        }
    }
    let mut x = vec![C64::new(0.0, 0.0); n];
    for r in (0..n).rev() {
        let s: C64 = (r + 1..n).map(|k| a[r * n + k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r * n + r];
    }
    Some(x)
}

fn null_vector(a: &[C64], rows: usize, m: usize) -> Vec<C64> {
    let mut g = vec![C64::new(0.0, 0.0); m * m];
    for i in 0..m {
        for j in 0..m {
            g[i * m + j] = (0..rows).map(|r| a[r * m + i].conj() * a[r * m + j]).sum();
        }
    }
    let trace: f64 = (0..m).map(|i| g[i * m + i].re).sum();
    let shift = 1e-13 * trace.max(1e-300);
    for i in 0..m {
        g[i * m + i] += shift;
    }
    let mut x: Vec<C64> = (0..m).map(|i| C64::new(1.0 + 0.1 * i as f64, 0.3)).collect();
    for _ in 0..40 {
        let Some(y) = solve_small(g.clone(), x.clone(), m) else {
            break;
        };
        let n = y.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt();
        if !(n.is_finite() && n > 0.0) {
            break;
        }
        x = y.into_iter().map(|c| c / n).collect();
    }
    x
}

impl Rational {
    pub fn aaa(x: &[f64], f: &[C64], tol: f64, max_terms: usize) -> Rational {
        let n = x.len();
        let scale = f.iter().map(|c| c.norm()).fold(0.0, f64::max).max(1e-300);
        let mean = f.iter().sum::<C64>() / n.max(1) as f64;
        let mut r: Vec<C64> = vec![mean; n];
        let mut chosen: Vec<usize> = Vec::new();
        let mut weights = Vec::new();
        for _ in 0..max_terms.min(n) {
            let j = (0..n)
                .filter(|i| !chosen.contains(i))
                .max_by(|&a, &b| (f[a] - r[a]).norm().total_cmp(&(f[b] - r[b]).norm()));
            let Some(j) = j else {
                break;
            };
            chosen.push(j);
            let rest: Vec<usize> = (0..n).filter(|i| !chosen.contains(i)).collect();
            let m = chosen.len();
            let mut a = vec![C64::new(0.0, 0.0); rest.len() * m];
            for (ri, &i) in rest.iter().enumerate() {
                for (k, &s) in chosen.iter().enumerate() {
                    a[ri * m + k] = (f[i] - f[s]) / (x[i] - x[s]);
                }
            }
            weights = if rest.len() < m {
                let mut w = null_vector(&a, rest.len(), m);
                if rest.is_empty() {
                    w = vec![C64::new(1.0, 0.0); m];
                }
                w
            } else {
                null_vector(&a, rest.len(), m)
            };
            let cur = Rational {
                support: chosen.iter().map(|&s| (x[s], f[s])).collect(),
                weights: weights.clone(),
            };
            r = x.iter().map(|&xi| cur.eval(xi)).collect();
            let err = (0..n).map(|i| (f[i] - r[i]).norm()).fold(0.0, f64::max);
            if err <= tol * scale {
                break;
            }
        }
        Rational { support: chosen.iter().map(|&s| (x[s], f[s])).collect(), weights }
    }

    pub fn eval(&self, x: f64) -> C64 {
        let mut num = C64::new(0.0, 0.0);
        let mut den = C64::new(0.0, 0.0);
        for ((xs, fs), w) in self.support.iter().zip(&self.weights) {
            let d = x - xs;
            if d.abs() < 1e-14 {
                return *fs;
            }
            num += w * fs / d;
            den += w / d;
        }
        if den.norm() == 0.0 { C64::new(f64::NAN, f64::NAN) } else { num / den }
    }

    pub fn terms(&self) -> usize {
        self.support.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resonator_is_recovered_from_a_handful_of_samples() {
        let z = |x: f64| {
            let w = 1.0 + 0.2 * x;
            C64::new(40.0, 0.0)
                + C64::new(0.0, 900.0 * (w - 1.0 / w))
                + 1.0 / C64::new(1.0 / 3000.0, 0.02 * (w - 1.3) * 50.0)
        };
        let xs: Vec<f64> = (0..12).map(|i| -1.0 + 2.0 * i as f64 / 11.0).collect();
        let fs: Vec<C64> = xs.iter().map(|&x| z(x)).collect();
        let r = Rational::aaa(&xs, &fs, 1e-12, 12);
        for i in 0..200 {
            let x = -1.0 + 2.0 * i as f64 / 199.0;
            let (a, b) = (r.eval(x), z(x));
            assert!((a - b).norm() < 1e-6 * b.norm().max(1.0), "{x} {a} {b}");
        }
    }
}
