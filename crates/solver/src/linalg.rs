use num_complex::Complex64 as C64;
use rayon::prelude::*;

pub struct System {
    pub a: Vec<C64>,
    pub n: usize,
    pub w: usize,
}

impl System {
    pub fn zeros(n: usize) -> Self {
        let w = n + 1;
        Self { a: vec![C64::new(0.0, 0.0); n * w], n, w }
    }

    pub fn rhs_mut(&mut self, row: usize) -> &mut C64 {
        &mut self.a[row * self.w + self.n]
    }
}

pub type Currents = Vec<C64>;

pub fn back_substitute(sys: &System) -> Currents {
    let (n, w, a) = (sys.n, sys.w, &sys.a);
    let mut x = vec![C64::new(0.0, 0.0); n];
    for r in (0..n).rev() {
        let row = &a[r * w..(r + 1) * w];
        let mut s = row[n];
        for j in r + 1..n {
            s -= row[j] * x[j];
        }
        let p = row[r];
        x[r] = if p.norm_sqr() > 0.0 { s / p } else { s / C64::new(1e-30, 0.0) };
    }
    x
}

pub fn solve_system(mut sys: System) -> Currents {
    let (n, w) = (sys.n, sys.w);
    for c in 0..n {
        let pivot = (c..n)
            .max_by(|&x, &y| sys.a[x * w + c].norm().total_cmp(&sys.a[y * w + c].norm()))
            .unwrap_or(c);
        if pivot != c {
            for j in c..w {
                sys.a.swap(c * w + j, pivot * w + j);
            }
        }
        let (head, tail) = sys.a.split_at_mut((c + 1) * w);
        let prow = &head[c * w..(c + 1) * w];
        let p = prow[c];
        if p.norm_sqr() == 0.0 {
            continue;
        }
        tail[..(n - c - 1) * w].par_chunks_mut(w).for_each(|row| {
            let f = row[c] / p;
            if f.re == 0.0 && f.im == 0.0 {
                return;
            }
            for j in c..w {
                row[j] -= f * prow[j];
            }
        });
    }
    back_substitute(&sys)
}
