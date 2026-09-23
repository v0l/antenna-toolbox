use crate::C64;
use std::f64::consts::PI;

pub fn bessel_j0(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let a = 57568490574.0
            + y * (-13362590354.0
                + y * (651619640.7 + y * (-11214424.18 + y * (77392.33017 + y * -184.9052456))));
        let b = 57568490411.0
            + y * (1029532985.0 + y * (9494680.718 + y * (59272.64853 + y * (267.8532712 + y))));
        a / b
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - std::f64::consts::FRAC_PI_4;
        let a = 1.0
            + y * (-0.1098628627e-2
                + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let b = -0.1562499995e-1
            + y * (0.1430488765e-3
                + y * (-0.6911147651e-5 + y * (0.7621095161e-6 - y * 0.934935152e-7)));
        (std::f64::consts::FRAC_2_PI / ax).sqrt() * (xx.cos() * a - z * xx.sin() * b)
    }
}

pub fn bessel_j1(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let a = x
            * (72362614232.0
                + y * (-7895059235.0
                    + y * (242396853.1
                        + y * (-2972611.439 + y * (15704.48260 + y * -30.16036606)))));
        let b = 144725228442.0
            + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        a / b
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 3.0 * std::f64::consts::FRAC_PI_4;
        let a = 1.0
            + y * (0.183105e-2
                + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * -0.240337019e-6)));
        let b = 0.04687499995
            + y * (-0.2002690873e-3
                + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        let v = (std::f64::consts::FRAC_2_PI / ax).sqrt() * (xx.cos() * a - z * xx.sin() * b);
        if x < 0.0 { -v } else { v }
    }
}

const GK_X: [f64; 8] = [
    0.9914553711208126,
    0.9491079123427585,
    0.8648644233597691,
    0.7415311855993945,
    0.5860872354676911,
    0.4058451513773972,
    0.20778495500789848,
    0.0,
];
const GK_WK: [f64; 8] = [
    0.022935322010529224,
    0.06309209262997856,
    0.10479001032225019,
    0.14065325971552592,
    0.1690047266392679,
    0.19035057806478542,
    0.20443294007529889,
    0.20948214108472782,
];
const GK_WG: [f64; 4] =
    [0.1294849661688697, 0.27970539148927664, 0.3818300505051189, 0.4179591836734694];

type Vals = [C64; 5];

fn zero() -> Vals {
    [C64::new(0.0, 0.0); 5]
}

fn add(a: &mut Vals, b: &Vals, s: f64) {
    for i in 0..5 {
        a[i] += b[i] * s;
    }
}

fn kronrod(f: &dyn Fn(f64) -> Vals, a: f64, b: f64) -> (Vals, f64) {
    let (c, h) = ((a + b) / 2.0, (b - a) / 2.0);
    let mut k = zero();
    let mut g = zero();
    for i in 0..8 {
        let x = GK_X[i] * h;
        if i == 7 {
            let v = f(c);
            add(&mut k, &v, GK_WK[7]);
            add(&mut g, &v, GK_WG[3]);
            continue;
        }
        let (l, r) = (f(c - x), f(c + x));
        add(&mut k, &l, GK_WK[i]);
        add(&mut k, &r, GK_WK[i]);
        if i % 2 == 1 {
            add(&mut g, &l, GK_WG[i / 2]);
            add(&mut g, &r, GK_WG[i / 2]);
        }
    }
    for v in k.iter_mut().chain(g.iter_mut()) {
        *v *= h;
    }
    let err = k.iter().zip(&g).map(|(a, b)| (a - b).norm()).fold(0.0, f64::max);
    (k, err)
}

fn adaptive(f: &dyn Fn(f64) -> Vals, a: f64, b: f64, tol: f64, depth: u32) -> Vals {
    let (v, err) = kronrod(f, a, b);
    let scale = v.iter().map(|x| x.norm()).fold(0.0, f64::max);
    if err <= tol.max(1e-12 * scale) || depth == 0 {
        return v;
    }
    let m = (a + b) / 2.0;
    let mut out = adaptive(f, a, m, tol / 2.0, depth - 1);
    add(&mut out, &adaptive(f, m, b, tol / 2.0, depth - 1), 1.0);
    out
}

pub struct Ground {
    pub eps: C64,
    pub k: f64,
    pub r0: C64,
}

impl Ground {
    pub fn new(eps: C64, k: f64) -> Self {
        Ground { eps, k, r0: (eps - 1.0) / (eps + 1.0) }
    }

    fn kernel(
        &self,
        lambda: C64,
        u1: C64,
        rho: f64,
        zeta: f64,
    ) -> (C64, C64, C64, C64, C64, f64, f64) {
        let eps = self.eps;
        let k2 = eps * self.k * self.k;
        let u2 = (lambda * lambda - k2).sqrt();
        let rv = (eps * u1 - u2) / (eps * u1 + u2);
        let rh = (u1 - u2) / (u1 + u2);
        let a = -(lambda * lambda) * 2.0 * (eps - 1.0) / ((u1 + u2) * (eps * u1 + u2));
        let e = (-u1 * zeta).exp();
        let x = lambda.re * rho;
        (rv, rh, a, e, u2, bessel_j0(x), bessel_j1(x))
    }

    fn integrand_below(&self, theta: f64, rho: f64, zeta: f64) -> Vals {
        let k = self.k;
        let lambda = C64::new(k * theta.sin(), 0.0);
        let u1 = C64::new(0.0, k * theta.cos());
        let dl = k * theta.cos();
        let over_u1 = C64::new(0.0, -k * theta.sin());
        let (rv, rh, a, e, _, j0, j1) = self.kernel(lambda, u1, rho, zeta);
        let r0 = self.r0;
        let lam = lambda.re;
        let stat = (-lam * zeta).exp();
        [
            (rv - r0) * j0 * e * over_u1,
            -(rv - r0) * j0 * lam * e * dl,
            rh * j0 * e * over_u1,
            j1 * (a * e + r0 * stat) * dl,
            -(j1 * (lam * rh * e * over_u1 + (u1 * a * e + r0 * lam * stat) * dl)),
        ]
    }

    fn integrand_above(&self, lam: f64, rho: f64, zeta: f64) -> Vals {
        let lambda = C64::new(lam, 0.0);
        let u1 = C64::new((lam * lam - self.k * self.k).max(0.0).sqrt(), 0.0);
        let (rv, rh, a, e, _, j0, j1) = self.kernel(lambda, u1, rho, zeta);
        let r0 = self.r0;
        let stat = (-lam * zeta).exp();
        let over_u1 = lam / u1;
        [
            (rv - r0) * j0 * e * over_u1,
            -(rv - r0) * j0 * lam * e,
            rh * j0 * e * over_u1,
            j1 * (a * e + r0 * stat),
            -(j1 * (lam * rh * e * over_u1 + u1 * a * e + r0 * lam * stat)),
        ]
    }

    fn integrand_hyper(&self, s: f64, rho: f64, zeta: f64) -> Vals {
        let k = self.k;
        let lam = k * s.cosh();
        let u1v = k * s.sinh();
        let dl = k * s.sinh();
        let lambda = C64::new(lam, 0.0);
        let u1 = C64::new(u1v, 0.0);
        let (rv, rh, a, e, _, j0, j1) = self.kernel(lambda, u1, rho, zeta);
        let r0 = self.r0;
        let stat = (-lam * zeta).exp();
        let over_u1 = k * s.cosh();
        [
            (rv - r0) * j0 * e * over_u1,
            -(rv - r0) * j0 * lam * e * dl,
            rh * j0 * e * over_u1,
            j1 * (a * e + r0 * stat) * dl,
            -(j1 * (lam * rh * e * over_u1 + (u1 * a * e + r0 * lam * stat) * dl)),
        ]
    }

    pub fn integrals(&self, rho: f64, zeta: f64) -> Vals {
        let k = self.k;
        let tol = 1e-9;
        let mut out = adaptive(&|t| self.integrand_below(t, rho, zeta), 0.0, PI / 2.0, tol, 18);
        let s_end = 2f64.acosh();
        add(&mut out, &adaptive(&|s| self.integrand_hyper(s, rho, zeta), 0.0, s_end, tol, 18), 1.0);
        let start = 2.0 * k;
        let width = if rho > 1e-9 { (PI / rho).min(4.0 * k) } else { 4.0 * k };
        let width = if zeta > 0.0 { width.min(2.0 / zeta) } else { width };
        let mut sums: Vec<Vals> = Vec::new();
        let mut total = zero();
        let mut a = start;
        for i in 0..4000 {
            let b = a + width;
            let (part, _) = kronrod(&|l| self.integrand_above(l, rho, zeta), a, b);
            add(&mut total, &part, 1.0);
            sums.push(total);
            a = b;
            let size = part.iter().map(|x| x.norm()).fold(0.0, f64::max);
            let scale = total.iter().map(|x| x.norm()).fold(0.0, f64::max).max(1e-300);
            if i > 4 && size < 1e-12 * scale {
                break;
            }
            if i > 8
                && let Some(v) = shanks(&sums[sums.len() - 9..])
                && let Some(p) = shanks(&sums[sums.len() - 10..sums.len() - 1])
            {
                let d = v.iter().zip(&p).map(|(x, y)| (x - y).norm()).fold(0.0, f64::max);
                if d < 1e-9 * scale {
                    total = v;
                    break;
                }
            }
        }
        add(&mut out, &total, 1.0);
        out
    }
}

fn shanks(s: &[Vals]) -> Option<Vals> {
    let mut out = zero();
    for c in 0..5 {
        let mut e: Vec<C64> = s.iter().map(|v| v[c]).collect();
        let mut prev: Vec<C64> = vec![C64::new(0.0, 0.0); e.len() + 1];
        let mut last = e.clone();
        let mut step = 0;
        while e.len() > 1 {
            let next: Vec<C64> = (0..e.len() - 1)
                .map(|i| {
                    let d = e[i + 1] - e[i];
                    if d.norm() < 1e-300 { C64::new(1e300, 0.0) } else { prev[i + 1] + d.inv() }
                })
                .collect();
            prev = e;
            e = next;
            step += 1;
            if step % 2 == 0 {
                last = e.clone();
            }
        }
        out[c] = *last.last()?;
        if !out[c].is_finite() {
            return None;
        }
    }
    Some(out)
}
