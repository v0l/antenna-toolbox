#![allow(clippy::needless_range_loop)]

pub struct SymEig {
    pub values: Vec<f64>,
    pub vectors: Vec<f64>,
    pub n: usize,
}

impl SymEig {
    pub fn vector(&self, k: usize) -> Vec<f64> {
        (0..self.n).map(|i| self.vectors[i * self.n + k]).collect()
    }
}

pub fn sym_eig(a: &[f64], n: usize) -> SymEig {
    let mut v: Vec<f64> = a.to_vec();
    let mut d = vec![0.0; n];
    let mut e = vec![0.0; n];
    if n == 0 {
        return SymEig { values: d, vectors: v, n };
    }
    tred2(&mut v, &mut d, &mut e, n);
    tql2(&mut v, &mut d, &mut e, n);
    SymEig { values: d, vectors: v, n }
}

fn tred2(v: &mut [f64], d: &mut [f64], e: &mut [f64], n: usize) {
    let at = |i: usize, j: usize| i * n + j;
    for j in 0..n {
        d[j] = v[at(n - 1, j)];
    }
    for i in (1..n).rev() {
        let mut scale = 0.0;
        let mut h = 0.0;
        for k in 0..i {
            scale += d[k].abs();
        }
        if scale == 0.0 {
            e[i] = d[i - 1];
            for j in 0..i {
                d[j] = v[at(i - 1, j)];
                v[at(i, j)] = 0.0;
                v[at(j, i)] = 0.0;
            }
        } else {
            for k in 0..i {
                d[k] /= scale;
                h += d[k] * d[k];
            }
            let mut f = d[i - 1];
            let mut g = h.sqrt();
            if f > 0.0 {
                g = -g;
            }
            e[i] = scale * g;
            h -= f * g;
            d[i - 1] = f - g;
            for j in 0..i {
                e[j] = 0.0;
            }
            for j in 0..i {
                f = d[j];
                v[at(j, i)] = f;
                g = e[j] + v[at(j, j)] * f;
                for k in j + 1..i {
                    g += v[at(k, j)] * d[k];
                    e[k] += v[at(k, j)] * f;
                }
                e[j] = g;
            }
            f = 0.0;
            for j in 0..i {
                e[j] /= h;
                f += e[j] * d[j];
            }
            let hh = f / (h + h);
            for j in 0..i {
                e[j] -= hh * d[j];
            }
            for j in 0..i {
                f = d[j];
                g = e[j];
                for k in j..i {
                    v[at(k, j)] -= f * e[k] + g * d[k];
                }
                d[j] = v[at(i - 1, j)];
                v[at(i, j)] = 0.0;
            }
        }
        d[i] = h;
    }
    for i in 0..n - 1 {
        v[at(n - 1, i)] = v[at(i, i)];
        v[at(i, i)] = 1.0;
        let h = d[i + 1];
        if h != 0.0 {
            for k in 0..=i {
                d[k] = v[at(k, i + 1)] / h;
            }
            for j in 0..=i {
                let mut g = 0.0;
                for k in 0..=i {
                    g += v[at(k, i + 1)] * v[at(k, j)];
                }
                for k in 0..=i {
                    v[at(k, j)] -= g * d[k];
                }
            }
        }
        for k in 0..=i {
            v[at(k, i + 1)] = 0.0;
        }
    }
    for j in 0..n {
        d[j] = v[at(n - 1, j)];
        v[at(n - 1, j)] = 0.0;
    }
    v[at(n - 1, n - 1)] = 1.0;
    e[0] = 0.0;
}

fn tql2(v: &mut [f64], d: &mut [f64], e: &mut [f64], n: usize) {
    let at = |i: usize, j: usize| i * n + j;
    for i in 1..n {
        e[i - 1] = e[i];
    }
    e[n - 1] = 0.0;
    let mut f = 0.0;
    let mut tst1: f64 = 0.0;
    let eps = f64::EPSILON;
    for l in 0..n {
        tst1 = tst1.max(d[l].abs() + e[l].abs());
        let mut m = l;
        while m < n {
            if e[m].abs() <= eps * tst1 {
                break;
            }
            m += 1;
        }
        let m = m.min(n - 1);
        if m > l {
            let mut iter = 0;
            loop {
                iter += 1;
                let mut g = d[l];
                let mut p = (d[l + 1] - g) / (2.0 * e[l]);
                let mut r = p.hypot(1.0);
                if p < 0.0 {
                    r = -r;
                }
                d[l] = e[l] / (p + r);
                d[l + 1] = e[l] * (p + r);
                let dl1 = d[l + 1];
                let mut h = g - d[l];
                for i in l + 2..n {
                    d[i] -= h;
                }
                f += h;
                p = d[m];
                let mut c = 1.0;
                let mut c2 = c;
                let mut c3 = c;
                let el1 = e[l + 1];
                let mut s = 0.0;
                let mut s2 = 0.0;
                for i in (l..m).rev() {
                    c3 = c2;
                    c2 = c;
                    s2 = s;
                    g = c * e[i];
                    h = c * p;
                    r = p.hypot(e[i]);
                    e[i + 1] = s * r;
                    s = e[i] / r;
                    c = p / r;
                    p = c * d[i] - s * g;
                    d[i + 1] = h + s * (c * g + s * d[i]);
                    for k in 0..n {
                        h = v[at(k, i + 1)];
                        v[at(k, i + 1)] = s * v[at(k, i)] + c * h;
                        v[at(k, i)] = c * v[at(k, i)] - s * h;
                    }
                }
                p = -s * s2 * c3 * el1 * e[l] / dl1;
                e[l] = s * p;
                d[l] = c * p;
                if e[l].abs() <= eps * tst1 || iter > 60 {
                    break;
                }
            }
        }
        d[l] += f;
        e[l] = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eigenpairs_satisfy_the_definition() {
        let n = 40;
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                let v = ((i * 7 + j * 3) % 11) as f64 - 5.0 + if i == j { 3.0 } else { 0.0 };
                a[i * n + j] = v;
                a[j * n + i] = v;
            }
        }
        let e = sym_eig(&a, n);
        for k in 0..n {
            let x = e.vector(k);
            for i in 0..n {
                let ax: f64 = (0..n).map(|j| a[i * n + j] * x[j]).sum();
                assert!((ax - e.values[k] * x[i]).abs() < 1e-9, "{k} {i}");
            }
            let norm: f64 = x.iter().map(|v| v * v).sum();
            assert!((norm - 1.0).abs() < 1e-9);
        }
    }
}
