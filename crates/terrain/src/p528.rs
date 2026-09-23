use std::f64::consts::PI;

const A_0: f64 = 6371.0;
const A_E: f64 = 9257.0;
const N_S: f64 = 341.0;
const EPSILON_R: f64 = 15.0;
const SIGMA: f64 = 0.005;
const THIRD: f64 = 1.0 / 3.0;
const Y_PI_99_INDEX: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    LineOfSight,
    Diffraction,
    Troposcatter,
}

#[derive(Clone, Copy, Debug)]
pub struct Result {
    pub loss_db: f64,
    pub free_space_db: f64,
    pub absorption_db: f64,
    pub mode: Mode,
    pub takeoff_rad: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Distance,
    LowTerminal,
    HighTerminal,
    Order,
    Frequency,
    Percent,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::Distance => "distance must not be negative",
            Error::LowTerminal => "the low terminal must be 1.5 m to 80 km up",
            Error::HighTerminal => "the high terminal must be 1.5 m to 80 km up",
            Error::Order => "the first terminal must be the lower one",
            Error::Frequency => "P.528 covers 100 MHz to 30 GHz",
            Error::Percent => "time percentage must be 1 to 99",
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Slant {
    a_gas: f64,
    bending: f64,
    a: f64,
    angle: f64,
}

fn geopotential(h: f64) -> f64 {
    6356.766 * h / (6356.766 + h)
}

fn temperature(h: f64) -> f64 {
    if h < 86.0 {
        let hp = geopotential(h);
        if hp <= 11.0 {
            288.15 - 6.5 * hp
        } else if hp <= 20.0 {
            216.65
        } else if hp <= 32.0 {
            216.65 + (hp - 20.0)
        } else if hp <= 47.0 {
            228.65 + 2.8 * (hp - 32.0)
        } else if hp <= 51.0 {
            270.65
        } else if hp <= 71.0 {
            270.65 - 2.8 * (hp - 51.0)
        } else {
            214.65 - 2.0 * (hp - 71.0)
        }
    } else if h <= 91.0 {
        186.8673
    } else {
        263.1905 - 76.3232 * (1.0 - ((h - 91.0) / 19.9429).powi(2)).sqrt()
    }
}

fn pressure(h: f64) -> f64 {
    if h < 86.0 {
        let hp = geopotential(h);
        if hp <= 11.0 {
            1013.25 * (288.15 / (288.15 - 6.5 * hp)).powf(-34.1632 / 6.5)
        } else if hp <= 20.0 {
            226.3226 * (-34.1632 * (hp - 11.0) / 216.65).exp()
        } else if hp <= 32.0 {
            54.74980 * (216.65 / (216.65 + (hp - 20.0))).powf(34.1632)
        } else if hp <= 47.0 {
            8.680422 * (228.65 / (228.65 + 2.8 * (hp - 32.0))).powf(34.1632 / 2.8)
        } else if hp <= 51.0 {
            1.109106 * (-34.1632 * (hp - 47.0) / 270.65).exp()
        } else if hp <= 71.0 {
            0.6694167 * (270.65 / (270.65 - 2.8 * (hp - 51.0))).powf(-34.1632 / 2.8)
        } else {
            0.03956649 * (214.65 / (214.65 - 2.0 * (hp - 71.0))).powf(-34.1632 / 2.0)
        }
    } else {
        (95.571899 - 4.011801 * h + 6.424731e-2 * h * h - 4.789660e-4 * h.powi(3)
            + 1.340543e-6 * h.powi(4))
        .exp()
    }
}

fn wet_pressure(h: f64) -> f64 {
    let t = temperature(h);
    let p = pressure(h);
    let rho = (7.5 * (-h / 2.0).exp()).max(2e-6 * 216.7 * p / t);
    rho * t / 216.7
}

fn refractive_index(p: f64, t: f64, e: f64) -> f64 {
    1.0 + (77.6 * p / t + 72.0 * e / t + 3.75e5 * e / (t * t)) * 1e-6
}

fn line_shape(f: f64, fi: f64, df: f64, delta: f64) -> f64 {
    f / fi
        * ((df - delta * (fi - f)) / ((fi - f).powi(2) + df * df)
            + (df - delta * (fi + f)) / ((fi + f).powi(2) + df * df))
}

fn oxygen_refractivity(f: f64, t: f64, e: f64, p: f64) -> f64 {
    let theta = 300.0 / t;
    let mut n = 0.0;
    for i in 0..O2_F_0.len() {
        let s = O2_A_1[i] * 1e-7 * p * theta.powi(3) * (O2_A_2[i] * (1.0 - theta)).exp();
        let df = O2_A_3[i] * 1e-4 * (p * theta.powf(0.8 - O2_A_4[i]) + 1.1 * e * theta);
        let df = (df * df + 2.25e-6).sqrt();
        let delta = (O2_A_5[i] + O2_A_6[i] * theta) * 1e-4 * (p + e) * theta.powf(0.8);
        n += s * line_shape(f, O2_F_0[i], df, delta);
    }
    let d = 5.6e-4 * (p + e) * theta.powf(0.8);
    let frac_1 = 6.14e-5 / (d * (1.0 + (f / d).powi(2)));
    let frac_2 = 1.4e-12 * p * theta.powf(1.5) / (1.0 + 1.9e-5 * f.powf(1.5));
    n + f * p * theta * theta * (frac_1 + frac_2)
}

fn water_refractivity(f: f64, t: f64, e: f64, p: f64) -> f64 {
    let theta = 300.0 / t;
    let mut n = 0.0;
    for i in 0..H2O_F_0.len() {
        let s = 0.1 * H2O_B_1[i] * e * theta.powf(3.5) * (H2O_B_2[i] * (1.0 - theta)).exp();
        let df = 1e-4
            * H2O_B_3[i]
            * (p * theta.powf(H2O_B_4[i]) + H2O_B_5[i] * e * theta.powf(H2O_B_6[i]));
        let term = 0.217 * df * df + 2.1316e-12 * H2O_F_0[i] * H2O_F_0[i] / theta;
        let df = 0.535 * df + term.sqrt();
        n += s * line_shape(f, H2O_F_0[i], df, 0.0);
    }
    n
}

fn layer(f: f64, h: f64) -> (f64, f64) {
    let (t, p, e) = (temperature(h), pressure(h), wet_pressure(h));
    let gamma = 0.1820 * f * (oxygen_refractivity(f, t, e, p) + water_refractivity(f, t, e, p));
    (refractive_index(p, t, e), gamma)
}

fn ray_trace(f: f64, h_1: f64, h_2: f64, beta_1: f64) -> Slant {
    let e1 = (1.0f64 / 100.0).exp();
    let i_lower = (100.0 * (1e4 * h_1 * (e1 - 1.0) + 1.0).ln() + 1.0).floor() as i64;
    let i_upper = (100.0 * (1e4 * h_2 * (e1 - 1.0) + 1.0).ln() + 1.0).ceil() as i64;
    let m = (((2.0f64 / 100.0).exp() - e1)
        / ((i_upper as f64 / 100.0).exp() - (i_lower as f64 / 100.0).exp()))
        * (h_2 - h_1);
    let thickness = |i: i64| m * ((i - 1) as f64 / 100.0).exp();
    let height = |i: i64| {
        h_1 + m * ((((i - 1) as f64) / 100.0).exp() - (((i_lower - 1) as f64) / 100.0).exp())
            / (e1 - 1.0)
    };
    let mut out = Slant::default();
    let mut delta_i = thickness(i_lower);
    let mut h_i = height(i_lower);
    let (mut n_i, mut gamma_i) = layer(f, h_i + delta_i / 2.0);
    let mut r_i = A_0 + h_i;
    let (r_1, n_1) = (r_i, n_i);
    let mut alpha_i = beta_1;
    for i in i_lower..i_upper {
        let delta_ii = thickness(i + 1);
        let h_ii = height(i + 1);
        let (n_ii, gamma_ii) = layer(f, h_ii + delta_ii / 2.0);
        let r_ii = A_0 + h_ii;
        delta_i = thickness(i);
        let beta_i = ((n_1 * r_1) / (n_i * r_i) * beta_1.sin()).min(1.0).asin();
        alpha_i = ((n_1 * r_1) / (n_i * r_ii) * beta_1.sin()).min(1.0).asin();
        let a_i = -r_i * beta_i.cos()
            + (r_i * r_i * beta_i.cos().powi(2) + 2.0 * r_i * delta_i + delta_i * delta_i).sqrt();
        out.a += a_i;
        out.a_gas += a_i * gamma_i;
        let beta_ii = (n_i / n_ii * alpha_i.sin()).asin();
        if i != i_upper - 1 {
            out.bending += beta_ii - alpha_i;
        }
        h_i = h_ii;
        n_i = n_ii;
        gamma_i = gamma_ii;
        r_i = r_ii;
    }
    let _ = h_i;
    out.angle = alpha_i;
    out
}

fn slant_path(f: f64, h_1: f64, h_2: f64, beta_1: f64) -> Slant {
    if beta_1 <= PI / 2.0 {
        return ray_trace(f, h_1, h_2, beta_1);
    }
    let n_1 = refractive_index(pressure(h_1), temperature(h_1), wet_pressure(h_1));
    let mut h_g = h_1;
    let mut delta = h_1 / 2.0;
    let mut diff = 100.0;
    loop {
        if diff > 0.0 {
            h_g -= delta;
        } else {
            h_g += delta;
        }
        delta /= 2.0;
        let n_g = refractive_index(pressure(h_g), temperature(h_g), wet_pressure(h_g));
        diff = n_g * (A_0 + h_g) - n_1 * (A_0 + h_1) * beta_1.sin();
        if diff.abs() <= 0.001 {
            break;
        }
    }
    let r1 = ray_trace(f, h_g, h_1, PI / 2.0);
    let r2 = ray_trace(f, h_g, h_2, PI / 2.0);
    Slant {
        angle: r2.angle,
        a_gas: r1.a_gas + r2.a_gas,
        a: r1.a + r2.a,
        bending: r1.bending + r2.bending,
    }
}

#[derive(Clone, Copy, Default)]
struct Terminal {
    h_r: f64,
    h_e: f64,
    delta_h: f64,
    d_r: f64,
    a: f64,
    theta: f64,
    a_a: f64,
}

fn terminal(f_mhz: f64, h_r: f64) -> Terminal {
    let s = slant_path(f_mhz / 1000.0, 0.0, h_r, PI / 2.0);
    let central = (PI / 2.0 - s.angle) + s.bending;
    let d_r = A_0 * central;
    let phi = d_r / A_E;
    let h_e = A_E / phi.cos() - A_E;
    Terminal { h_r, h_e, delta_h: h_r - h_e, d_r, a: s.a, theta: PI / 2.0 - s.angle, a_a: s.a_gas }
}

#[derive(Clone, Copy, Default)]
struct Los {
    d: f64,
    r_0: f64,
    r_12: f64,
    d_: [f64; 2],
    theta_h1: f64,
    a_a: f64,
    delta_r: f64,
    a_los: f64,
}

fn ray_optics(t1: &Terminal, t2: &Terminal, psi: f64) -> Los {
    let z = A_0 / A_E - 1.0;
    let k_a = 1.0 / (1.0 + z * psi.cos());
    let a_a = A_0 * k_a;
    let dh1 = t1.delta_h * (a_a - A_0) / (A_E - A_0);
    let dh2 = t2.delta_h * (a_a - A_0) / (A_E - A_0);
    let h = [t1.h_r - dh1, t2.h_r - dh2];
    let mut zz = [0.0; 2];
    let mut theta = [0.0; 2];
    let mut dd = [0.0; 2];
    let mut hp = [0.0; 2];
    for i in 0..2 {
        zz[i] = a_a + h[i];
        theta[i] = (a_a * psi.cos() / zz[i]).acos() - psi;
        dd[i] = zz[i] * theta[i].sin();
        hp[i] = if psi > 1.56 { h[i] } else { dd[i] * psi.tan() };
    }
    let delta_z = (zz[0] - zz[1]).abs();
    let d = (a_a * (theta[0] + theta[1])).max(0.0);
    let alpha = ((hp[1] - hp[0]) / (dd[0] + dd[1])).atan();
    let r_0 = delta_z.max((dd[0] + dd[1]) / alpha.cos());
    let r_12 = (dd[0] + dd[1]) / psi.cos();
    Los {
        d,
        r_0,
        r_12,
        d_: dd,
        theta_h1: alpha - theta[0],
        a_a,
        delta_r: 4.0 * hp[0] * hp[1] / (r_0 + r_12),
        a_los: 0.0,
    }
}

fn reflection(psi: f64, f_mhz: f64, vertical: bool) -> (f64, f64) {
    let (sin_psi, cos_psi) = if psi <= 0.0 {
        (0.0, 1.0)
    } else if psi >= PI / 2.0 {
        (1.0, 0.0)
    } else {
        (psi.sin(), psi.cos())
    };
    let x = 18000.0 * SIGMA / f_mhz;
    let y = EPSILON_R - cos_psi * cos_psi;
    let t = (y * y + x * x).sqrt() + y;
    let p = (t * 0.5).sqrt();
    let q = x / (2.0 * p);
    let pq = p * p + q * q;
    let (b, a) = if vertical {
        ((EPSILON_R * EPSILON_R + x * x) / pq, 2.0 * (p * EPSILON_R + q * x) / pq)
    } else {
        (1.0 / pq, 2.0 * p / pq)
    };
    let r_g = ((1.0 + b * sin_psi * sin_psi - a * sin_psi)
        / (1.0 + b * sin_psi * sin_psi + a * sin_psi))
        .sqrt();
    let (alpha, beta) = if vertical {
        (
            (EPSILON_R * sin_psi - q).atan2(EPSILON_R * sin_psi - p),
            (x * sin_psi + q).atan2(EPSILON_R * sin_psi + p),
        )
    } else {
        ((-q).atan2(sin_psi - p), q.atan2(sin_psi + p))
    };
    (r_g, alpha - beta)
}

struct Geometry {
    d_ml: f64,
    d_0: f64,
}

#[allow(clippy::too_many_arguments)]
fn path_loss_los(
    psi: f64,
    g: &Geometry,
    f_mhz: f64,
    psi_limit: f64,
    a_dml: f64,
    a_d_0: f64,
    vertical: bool,
    los: &mut Los,
) -> f64 {
    let (r_g, phi_g) = reflection(psi, f_mhz, vertical);
    let d_v = if psi.tan() >= 0.1 {
        1.0
    } else {
        let r_1 = los.d_[0] / psi.cos();
        let r_2 = los.d_[1] / psi.cos();
        let r_r = r_1 * r_2 / los.r_12;
        let term_1 = 2.0 * r_r * (1.0 + psi.sin().powi(2)) / (los.a_a * psi.sin());
        let term_2 = (2.0 * r_r / los.a_a).powi(2);
        (1.0 + term_1 + term_2).powf(-0.5)
    };
    let f_r = (los.r_0 / los.r_12).min(1.0);
    let r_tg = r_g * d_v * f_r;
    los.a_los = if los.d > g.d_0 {
        (los.d - g.d_0) * (a_dml - a_d_0) / (g.d_ml - g.d_0) + a_d_0
    } else {
        let lambda = 0.2997925 / f_mhz;
        if psi > psi_limit {
            0.0
        } else {
            let phi_tg = 2.0 * PI * los.delta_r / lambda + phi_g;
            let (re, im) = (1.0 + r_tg * phi_tg.cos(), -r_tg * phi_tg.sin());
            let w_rl = (re * re + im * im).sqrt().min(1.0);
            10.0 * (w_rl * w_rl).log10()
        }
    };
    r_tg
}

fn psi_at_distance(d: f64, t1: &Terminal, t2: &Terminal) -> f64 {
    if d == 0.0 {
        return PI / 2.0;
    }
    let mut psi = PI / 2.0;
    let mut delta = -PI / 4.0;
    loop {
        psi += delta;
        let dp = ray_optics(t1, t2, psi).d;
        delta = if dp > d { delta.abs() / 2.0 } else { -delta.abs() / 2.0 };
        if !((d - dp).abs() > 1e-3 && delta.abs() > 1e-12) {
            return psi;
        }
    }
}

fn search_delta_r(delta_r: f64, t1: &Terminal, t2: &Terminal, terminate: f64) -> (f64, Los) {
    let mut psi = PI / 2.0;
    let mut delta = -PI / 4.0;
    loop {
        psi += delta;
        let p = ray_optics(t1, t2, psi);
        delta = if p.delta_r > delta_r { -delta.abs() / 2.0 } else { delta.abs() / 2.0 };
        if (p.delta_r - delta_r).abs() <= terminate || delta.abs() < 1e-15 {
            return (psi, p);
        }
    }
}

fn icdf(q: f64) -> f64 {
    let x = if q > 0.5 { 1.0 - q } else { q };
    let t = (-2.0 * x.ln()).sqrt();
    let zeta = ((0.010328 * t + 0.802853) * t + 2.515516)
        / (((0.001308 * t + 0.189269) * t + 1.432788) * t + 1.0);
    if q > 0.5 { -(t - zeta) } else { t - zeta }
}

fn lerp(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    (y1 * (x2 - x) + y2 * (x - x1)) / (x2 - x1)
}

fn upper_bound(v: &[f64], x: f64) -> usize {
    v.iter().position(|&a| a > x).unwrap_or(v.len())
}

fn lower_bound(v: &[f64], x: f64) -> usize {
    v.iter().position(|&a| a >= x).unwrap_or(v.len())
}

fn long_term(
    d_r1: f64,
    d_r2: f64,
    d: f64,
    f_mhz: f64,
    p: f64,
    f_theta_h: f64,
    a_t: f64,
) -> (f64, f64) {
    let d_qs = 65.0 * (100.0 / f_mhz).powf(THIRD);
    let d_q = d_r1 + d_r2 + d_qs;
    let d_e = if d <= d_q { 130.0 * d / d_q } else { 130.0 + d - d_q };
    let (g_10, g_90) = if f_mhz > 1600.0 {
        (1.05, 1.05)
    } else {
        let s = (5.22 * (f_mhz / 200.0).log10()).sin();
        (0.21 * s + 1.28, 0.18 * s + 1.23)
    };
    let c_1 = [2.93e-4, 5.25e-4, 1.59e-5];
    let c_2 = [3.78e-8, 1.57e-6, 1.56e-11];
    let c_3 = [1.02e-7, 4.70e-7, 2.77e-8];
    let n_1 = [2.00, 1.97, 2.32];
    let n_2 = [2.88, 2.31, 4.08];
    let n_3 = [3.15, 2.90, 3.25];
    let f_inf = [3.2, 5.4, 0.0];
    let f_m = [8.2, 10.0, 3.9];
    let mut z = [0.0; 3];
    for i in 0..3 {
        let f_2 = f_inf[i] + (f_m[i] - f_inf[i]) * (-c_2[i] * d_e.powf(n_2[i])).exp();
        z[i] = (c_1[i] * d_e.powf(n_1[i]) - f_2) * (-c_3[i] * d_e.powf(n_3[i])).exp() + f_2;
    }
    let y_p = if p == 50.0 {
        z[2]
    } else if p > 50.0 {
        let c_p = icdf(p / 100.0) / icdf(0.9);
        c_p * (-z[0] * g_90) + z[2]
    } else {
        let c_p = if p >= 10.0 {
            icdf(p / 100.0) / icdf(0.1)
        } else {
            let ps = [1.0, 2.0, 5.0, 10.0];
            let cs = [1.9507, 1.7166, 1.3265, 1.0000];
            let k = upper_bound(&PERCENTS, p);
            lerp(ps[k - 1], cs[k - 1], ps[k], cs[k], p)
        };
        c_p * (z[1] * g_10) + z[2]
    };
    let y_10 = z[1] * g_10 + z[2];
    let a_y = (a_t + f_theta_h * y_10 - 3.0).max(0.0);
    let mut y_e = f_theta_h * y_p - a_y;
    if p < 10.0 {
        let c_y = [-5.0, -4.5, -3.7, 0.0];
        let k = upper_bound(&PERCENTS, p);
        let c_yi = lerp(PERCENTS[k - 1], c_y[k - 1], PERCENTS[k], c_y[k], p);
        y_e += a_t;
        if y_e > -c_yi {
            y_e = -c_yi;
        }
        y_e -= a_t;
    }
    (y_e, a_y)
}

fn nakagami_rice(k: f64, p: f64) -> f64 {
    let d_k = lower_bound(&K_VALUES, k);
    let d_p = lower_bound(&PERCENTS, p);
    let curve = |i: usize, j: usize| NAKAGAMI_RICE[i][j];
    if d_k == 0 {
        if d_p == 0 {
            curve(0, 0)
        } else {
            lerp(PERCENTS[d_p], curve(0, d_p), PERCENTS[d_p - 1], curve(0, d_p - 1), p)
        }
    } else if d_k == K_VALUES.len() {
        let i = d_k - 1;
        if d_p == 0 {
            curve(i, 0)
        } else {
            lerp(PERCENTS[d_p], curve(i, d_p), PERCENTS[d_p - 1], curve(i, d_p - 1), p)
        }
    } else if d_p == 0 {
        lerp(K_VALUES[d_k], curve(d_k, 0), K_VALUES[d_k - 1], curve(d_k - 1, 0), k)
    } else {
        let v1 = lerp(K_VALUES[d_k], curve(d_k, d_p), K_VALUES[d_k - 1], curve(d_k - 1, d_p), k);
        let v2 = lerp(
            K_VALUES[d_k],
            curve(d_k, d_p - 1),
            K_VALUES[d_k - 1],
            curve(d_k - 1, d_p) - 1.0,
            k,
        );
        lerp(PERCENTS[d_p], v1, PERCENTS[d_p - 1], v2, p)
    }
}

fn combine(a_m: f64, a_p: f64, b_m: f64, b_p: f64, p: f64) -> f64 {
    let c_m = a_m + b_m;
    let y_3 = ((a_p - a_m).powi(2) + (b_p - b_m).powi(2)).sqrt();
    if p < 50.0 { c_m + y_3 } else { c_m - y_3 }
}

fn k_for_y99(y: f64) -> f64 {
    if y < NAKAGAMI_RICE[0][Y_PI_99_INDEX] {
        return K_VALUES[0];
    }
    for i in 0..K_VALUES.len() {
        if y - NAKAGAMI_RICE[i][Y_PI_99_INDEX] < 0.0 {
            return (K_VALUES[i] * (y - NAKAGAMI_RICE[i - 1][Y_PI_99_INDEX])
                - K_VALUES[i - 1] * (y - NAKAGAMI_RICE[i][Y_PI_99_INDEX]))
                / (NAKAGAMI_RICE[i][Y_PI_99_INDEX] - NAKAGAMI_RICE[i - 1][Y_PI_99_INDEX]);
        }
    }
    K_VALUES[K_VALUES.len() - 1]
}

fn height_function(x: f64, k: f64) -> f64 {
    let y = 40.0 * x.log10() - 117.0;
    let g = 0.05751 * x - 10.0 * x.log10();
    if x <= 200.0 {
        let x_t = 450.0 / -(k.log10().powi(3));
        if x >= x_t {
            if y.abs() < 117.0 { y } else { -117.0 }
        } else {
            20.0 * k.log10() - 15.0 + 0.000025 * x * x / k
        }
    } else if x > 2000.0 {
        g
    } else {
        let w = 0.0134 * x * (-0.005 * x).exp();
        w * y + (1.0 - w) * g
    }
}

fn smooth_earth(d_1: f64, d_2: f64, f_mhz: f64, d_0: f64, vertical: bool) -> f64 {
    let s = 18000.0 * SIGMA / f_mhz;
    let k = if vertical {
        0.01778
            * f_mhz.powf(-THIRD)
            * ((EPSILON_R * EPSILON_R + s * s) / ((EPSILON_R - 1.0).powi(2) + s * s).powf(0.5))
                .powf(0.5)
    } else {
        0.01778 * f_mhz.powf(-THIRD) * ((EPSILON_R - 1.0).powi(2) + s * s).powf(-0.25)
    };
    let scale = (1.607 - k) * f_mhz.powf(THIRD);
    let (x_0, x_1, x_2) = (scale * d_0, scale * d_1, scale * d_2);
    let g = 0.05751 * x_0 - 10.0 * x_0.log10();
    g - height_function(x_1, k) - height_function(x_2, k) - 20.0
}

#[derive(Clone, Copy, Default)]
struct Tropo {
    a_s: f64,
    h_v: f64,
    theta_s: f64,
}

fn troposcatter(t1: &Terminal, t2: &Terminal, d: f64, f_mhz: f64) -> Tropo {
    let d_s = d - t1.d_r - t2.d_r;
    if d_s <= 0.0 {
        return Tropo::default();
    }
    let d_z = 0.5 * d_s;
    let a_m = 1.0 / A_0;
    let dn = a_m - 1.0 / A_E;
    let gamma_e = N_S * 1e-6 / dn;
    let z_a = 1.0 / (2.0 * A_E) * (d_z / 2.0).powi(2);
    let z_b = 1.0 / (2.0 * A_E) * d_z * d_z;
    let q = |z: f64| a_m - dn / (z / gamma_e).min(35.0).exp();
    let q_o = a_m - dn;
    let (q_a, q_b) = (q(z_a), q(z_b));
    let big_z_a = (7.0 * q_o + 6.0 * q_a - q_b) * (d_z * d_z / 96.0);
    let big_z_b = (q_o + 2.0 * q_a) * (d_z * d_z / 6.0);
    let (q_big_a, q_big_b) = (q(big_z_a), q(big_z_b));
    let h_v = (q_o + 2.0 * q_big_a) * (d_z * d_z / 6.0);
    let theta_a = (q_o + 4.0 * q_big_a + q_big_b) * d_z / 6.0;
    let theta_s = 2.0 * theta_a;
    let eps_1 = 5.67e-6 * N_S * N_S - 0.00232 * N_S + 0.031;
    let eps_2 = 0.0002 * N_S * N_S - 0.06 * N_S + 6.6;
    let gamma = 0.1424 * (1.0 + eps_1 / (h_v / 4.0).powi(6).min(35.0).exp());
    let s_e = 83.1 - eps_2 / (1.0 + 0.07716 * h_v * h_v)
        + 20.0 * ((0.1424 / gamma).powi(2) * (gamma * h_v).exp()).log10();
    let x_a = |t: &Terminal| {
        t.h_e * t.h_e + 4.0 * (A_E + t.h_e) * A_E * (t.d_r / (A_E * 2.0)).sin().powi(2)
    };
    let ell_1 = x_a(t1).sqrt() + d_z;
    let ell_2 = x_a(t2).sqrt() + d_z;
    let ell = ell_1 + ell_2;
    let s = (ell_1 - ell_2) / ell;
    let eta = gamma * theta_s * ell / 2.0;
    let kappa = f_mhz / 0.0477;
    let rho_1 = 2.0 * kappa * theta_s * t1.h_e;
    let rho_2 = 2.0 * kappa * theta_s * t2.h_e;
    let sqrt2 = 2f64.sqrt();
    let a = (1.0 - s * s).powi(2);
    let x_v1 = (1.0 + s).powi(2) * eta;
    let x_v2 = (1.0 - s).powi(2) * eta;
    let q_1 = x_v1 * x_v1 + rho_1 * rho_1;
    let q_2 = x_v2 * x_v2 + rho_2 * rho_2;
    let b_s = 6.0
        + 8.0 * s * s
        + 8.0 * (1.0 - s) * x_v1 * x_v1 * rho_1 * rho_1 / (q_1 * q_1)
        + 8.0 * (1.0 + s) * x_v2 * x_v2 * rho_2 * rho_2 / (q_2 * q_2)
        + 2.0 * (1.0 - s * s) * (1.0 + 2.0 * x_v1 * x_v1 / q_1) * (1.0 + 2.0 * x_v2 * x_v2 / q_2);
    let c_s = 12.0
        * ((rho_1 + sqrt2) / rho_1).powi(2)
        * ((rho_2 + sqrt2) / rho_2).powi(2)
        * (rho_1 + rho_2)
        / (rho_1 + rho_2 + 2.0 * sqrt2);
    let temp = (a * eta * eta + b_s * eta) * q_1 * q_2 / (rho_1 * rho_1 * rho_2 * rho_2);
    let s_v = 10.0 * (temp + c_s).log10();
    Tropo { a_s: s_e + s_v + 10.0 * (kappa * theta_s.powi(3) / ell).log10(), h_v, theta_s }
}

pub struct Model {
    f_mhz: f64,
    vertical: bool,
    t1: Terminal,
    t2: Terminal,
    g: Geometry,
    m_d: f64,
    a_d0: f64,
    a_dml: f64,
    psi_limit: f64,
    d_0_los: f64,
    beyond: Option<(f64, f64, f64, bool)>,
}

impl Model {
    pub fn new(
        h_1_m: f64,
        h_2_m: f64,
        f_mhz: f64,
        vertical: bool,
    ) -> std::result::Result<Model, Error> {
        if !(1.5..=80_000.0).contains(&h_1_m) {
            return Err(Error::LowTerminal);
        }
        if !(1.5..=80_000.0).contains(&h_2_m) {
            return Err(Error::HighTerminal);
        }
        if h_1_m > h_2_m {
            return Err(Error::Order);
        }
        if !(100.0..=30_000.0).contains(&f_mhz) {
            return Err(Error::Frequency);
        }
        let t1 = terminal(f_mhz, h_1_m / 1000.0);
        let t2 = terminal(f_mhz, h_2_m / 1000.0);
        let d_ml = t1.d_r + t2.d_r;
        let scale = (A_E * A_E / f_mhz).powf(THIRD);
        let d_3 = d_ml + 0.5 * scale;
        let d_4 = d_ml + 1.5 * scale;
        let a_3 = smooth_earth(t1.d_r, t2.d_r, f_mhz, d_3, vertical);
        let a_4 = smooth_earth(t1.d_r, t2.d_r, f_mhz, d_4, vertical);
        let m_d = (a_4 - a_3) / (d_4 - d_3);
        let a_d0 = a_4 - m_d * d_4;
        let a_dml = m_d * d_ml + a_d0;
        let d_d = -(a_d0 / m_d);
        let lambda = 0.2997925 / f_mhz;
        let terminate = lambda / 1e6;
        let psi_limit = search_delta_r(lambda / 2.0, &t1, &t2, terminate).0;
        let d_y6 = search_delta_r(lambda / 6.0, &t1, &t2, terminate).1.d;
        let mut g = Geometry { d_ml, d_0: 0.0 };
        g.d_0 = if t1.d_r >= d_d || d_d >= d_ml {
            if t1.d_r > d_y6 || d_y6 > d_ml { t1.d_r } else { d_y6 }
        } else if d_d < d_y6 && d_y6 < d_ml {
            d_y6
        } else {
            d_d
        };
        let mut d_temp = g.d_0;
        loop {
            let psi = psi_at_distance(d_temp, &t1, &t2);
            let r = ray_optics(&t1, &t2, psi);
            if r.d >= g.d_0 || d_temp + 0.001 >= d_ml {
                g.d_0 = r.d;
                break;
            }
            d_temp += 0.001;
        }
        let psi_d0 = psi_at_distance(g.d_0, &t1, &t2);
        let mut at_d0 = ray_optics(&t1, &t2, psi_d0);
        path_loss_los(psi_d0, &g, f_mhz, psi_limit, -a_dml, 0.0, vertical, &mut at_d0);
        let mut m = Model {
            f_mhz,
            vertical,
            t1,
            t2,
            g,
            m_d,
            a_d0,
            a_dml,
            psi_limit,
            d_0_los: at_d0.a_los,
            beyond: None,
        };
        m.beyond = Some(m.transhorizon());
        Ok(m)
    }

    fn transhorizon(&self) -> (f64, f64, f64, bool) {
        let (mut m_d, mut a_d0) = (self.m_d, self.a_d0);
        let mut k = 0;
        let mut d_search = [self.g.d_ml + 3.0, self.g.d_ml + 2.0];
        let mut a_s = [0.0, 0.0];
        for _ in 0..100 {
            a_s[1] = a_s[0];
            let t = troposcatter(&self.t1, &self.t2, d_search[0], self.f_mhz);
            a_s[0] = t.a_s;
            if t.a_s < 20.0 {
                d_search[1] = d_search[0];
                d_search[0] += 1.0;
                continue;
            }
            k += 1;
            if k <= 1 {
                d_search[1] = d_search[0];
                d_search[0] += 1.0;
                continue;
            }
            let m_s = (a_s[0] - a_s[1]) / (d_search[0] - d_search[1]);
            if m_s <= m_d {
                let d_crx = d_search[0];
                let a_d = m_d * d_search[1] + a_d0;
                if a_s[1] >= a_d {
                    return (m_d, a_d0, d_crx, true);
                }
                m_d = (a_s[1] - self.a_dml) / (d_search[1] - self.g.d_ml);
                a_d0 = a_s[1] - m_d * d_search[1];
                return (m_d, a_d0, d_crx, false);
            }
            d_search[1] = d_search[0];
            d_search[0] += 1.0;
        }
        (m_d, a_d0, d_search[1], true)
    }

    fn line_of_sight(&self, d: f64, p: f64) -> (Result, f64) {
        let lambda = 0.2997925 / self.f_mhz;
        let psi = psi_at_distance(d, &self.t1, &self.t2);
        let mut los = ray_optics(&self.t1, &self.t2, psi);
        let r_tg = path_loss_los(
            psi,
            &self.g,
            self.f_mhz,
            self.psi_limit,
            -self.a_dml,
            self.d_0_los,
            self.vertical,
            &mut los,
        );
        let slant =
            slant_path(self.f_mhz / 1000.0, self.t1.h_r, self.t2.h_r, PI / 2.0 - los.theta_h1);
        let a_a = slant.a_gas;
        let a_fs = 20.0 * los.r_0.log10() + 20.0 * self.f_mhz.log10() + 32.45;
        let f_theta_h = if los.theta_h1 <= 0.0 {
            1.0
        } else if los.theta_h1 >= 1.0 {
            0.0
        } else {
            (0.5 - (1.0 / PI) * (20.0 * (32.0 * los.theta_h1).log10()).atan()).max(0.0)
        };
        let (y_e, a_y) =
            long_term(self.t1.d_r, self.t2.d_r, d, self.f_mhz, p, f_theta_h, los.a_los);
        let (y_e_50, _) =
            long_term(self.t1.d_r, self.t2.d_r, d, self.f_mhz, 50.0, f_theta_h, los.a_los);
        let f_ay = if a_y <= 0.0 {
            1.0
        } else if a_y >= 9.0 {
            0.1
        } else {
            (1.1 + 0.9 * (a_y / 9.0 * PI).cos()) / 2.0
        };
        let f_delta_r = if los.delta_r >= lambda / 2.0 {
            1.0
        } else if los.delta_r <= lambda / 6.0 {
            0.1
        } else {
            0.5 * (1.1 - 0.9 * ((3.0 * PI / lambda) * (los.delta_r - lambda / 6.0)).cos())
        };
        let r_s = r_tg * f_delta_r * f_ay;
        let y_pi_99 = 10.0 * (self.f_mhz * slant.a.powi(3)).log10() - 84.26;
        let k_t = k_for_y99(y_pi_99);
        let w = r_s * r_s + 0.01 * 0.01 + 10f64.powf(k_t / 10.0);
        let k_los = if w <= 0.0 { -40.0 } else { (10.0 * w.log10()).max(-40.0) };
        let y_pi = nakagami_rice(k_los, p);
        let y_total = -combine(y_e_50, y_e, 0.0, y_pi, p);
        (
            Result {
                loss_db: a_fs + a_a - los.a_los + y_total,
                free_space_db: a_fs,
                absorption_db: a_a,
                mode: Mode::LineOfSight,
                takeoff_rad: los.theta_h1,
            },
            k_los,
        )
    }

    pub fn loss(&self, d_km: f64, p: f64) -> std::result::Result<Result, Error> {
        if d_km < 0.0 {
            return Err(Error::Distance);
        }
        if !(1.0..=99.0).contains(&p) {
            return Err(Error::Percent);
        }
        if self.g.d_ml - d_km > 0.001 {
            return Ok(self.line_of_sight(d_km, p).0);
        }
        let (_, k_los) = self.line_of_sight(self.g.d_ml - 1.0, p);
        let (m_d, a_d0, d_crx, case_1) =
            self.beyond.unwrap_or((self.m_d, self.a_d0, f64::INFINITY, true));
        let a_d = m_d * d_km + a_d0;
        let tropo = troposcatter(&self.t1, &self.t2, d_km, self.f_mhz);
        let (a_t, mode) = if d_km < d_crx {
            (a_d, Mode::Diffraction)
        } else if case_1 {
            if tropo.a_s <= a_d {
                (tropo.a_s, Mode::Troposcatter)
            } else {
                (a_d, Mode::Diffraction)
            }
        } else {
            (tropo.a_s, Mode::Troposcatter)
        };
        let (y_e, _) = long_term(self.t1.d_r, self.t2.d_r, d_km, self.f_mhz, p, 1.0, -a_t);
        let (y_e_50, _) = long_term(self.t1.d_r, self.t2.d_r, d_km, self.f_mhz, 50.0, 1.0, -a_t);
        let angle = 0.02617993878;
        let k_t = if tropo.theta_s >= angle {
            20.0
        } else if tropo.theta_s <= 0.0 {
            k_los
        } else {
            tropo.theta_s * (20.0 - k_los) / angle + k_los
        };
        let y_pi = nakagami_rice(k_t, p);
        let y_total = combine(y_e_50, y_e, 0.0, y_pi, p);
        let v = slant_path(self.f_mhz / 1000.0, 0.0, tropo.h_v, PI / 2.0);
        let a_a = self.t1.a_a + self.t2.a_a + 2.0 * v.a_gas;
        let r_fs = self.t1.a + self.t2.a + 2.0 * v.a;
        let a_fs = 20.0 * self.f_mhz.log10() + 20.0 * r_fs.log10() + 32.45;
        Ok(Result {
            loss_db: a_fs + a_a + a_t - y_total,
            free_space_db: a_fs,
            absorption_db: a_a,
            mode,
            takeoff_rad: -self.t1.theta,
        })
    }
}

pub fn p528(
    d_km: f64,
    h_1_m: f64,
    h_2_m: f64,
    f_mhz: f64,
    vertical: bool,
    p: f64,
) -> std::result::Result<Result, Error> {
    Model::new(h_1_m, h_2_m, f_mhz, vertical)?.loss(d_km, p)
}
const O2_F_0: [f64; 44] = [
    50.474214, 50.987745, 51.503360, 52.021429, 52.542418, 53.066934, 53.595775, 54.130025,
    54.671180, 55.221384, 55.783815, 56.264774, 56.363399, 56.968211, 57.612486, 58.323877,
    58.446588, 59.164204, 59.590983, 60.306056, 60.434778, 61.150562, 61.800158, 62.411220,
    62.486253, 62.997984, 63.568526, 64.127775, 64.678910, 65.224078, 65.764779, 66.302096,
    66.836834, 67.369601, 67.900868, 68.431006, 68.960312, 118.750334, 368.498246, 424.763020,
    487.249273, 715.392902, 773.839490, 834.145546,
];
const O2_A_1: [f64; 44] = [
    0.975, 2.529, 6.193, 14.320, 31.240, 64.290, 124.600, 227.300, 389.700, 627.100, 945.300,
    543.400, 1331.800, 1746.600, 2120.100, 2363.700, 1442.100, 2379.900, 2090.700, 2103.400,
    2438.000, 2479.500, 2275.900, 1915.400, 1503.000, 1490.200, 1078.000, 728.700, 461.300,
    274.000, 153.000, 80.400, 39.800, 18.560, 8.172, 3.397, 1.334, 940.300, 67.400, 637.700,
    237.400, 98.100, 572.300, 183.100,
];
const O2_A_2: [f64; 44] = [
    9.651, 8.653, 7.709, 6.819, 5.983, 5.201, 4.474, 3.800, 3.182, 2.618, 2.109, 0.014, 1.654,
    1.255, 0.910, 0.621, 0.083, 0.387, 0.207, 0.207, 0.386, 0.621, 0.910, 1.255, 0.083, 1.654,
    2.108, 2.617, 3.181, 3.800, 4.473, 5.200, 5.982, 6.818, 7.708, 8.652, 9.650, 0.010, 0.048,
    0.044, 0.049, 0.145, 0.141, 0.145,
];
const O2_A_3: [f64; 44] = [
    6.690, 7.170, 7.640, 8.110, 8.580, 9.060, 9.550, 9.960, 10.370, 10.890, 11.340, 17.030, 11.890,
    12.230, 12.620, 12.950, 14.910, 13.530, 14.080, 14.150, 13.390, 12.920, 12.630, 12.170, 15.130,
    11.740, 11.340, 10.880, 10.380, 9.960, 9.550, 9.060, 8.580, 8.110, 7.640, 7.170, 6.690, 16.640,
    16.400, 16.400, 16.000, 16.000, 16.200, 14.700,
];
const O2_A_4: [f64; 44] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];
const O2_A_5: [f64; 44] = [
    2.566, 2.246, 1.947, 1.667, 1.388, 1.349, 2.227, 3.170, 3.558, 2.560, -1.172, 3.525, -2.378,
    -3.545, -5.416, -1.932, 6.768, -6.561, 6.957, -6.395, 6.342, 1.014, 5.014, 3.029, -4.499,
    1.856, 0.658, -3.036, -3.968, -3.528, -2.548, -1.660, -1.680, -1.956, -2.216, -2.492, -2.773,
    -0.439, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000,
];
const O2_A_6: [f64; 44] = [
    6.850, 6.800, 6.729, 6.640, 6.526, 6.206, 5.085, 3.750, 2.654, 2.952, 6.135, -0.978, 6.547,
    6.451, 6.056, 0.436, -1.273, 2.309, -0.776, 0.699, -2.825, -0.584, -6.619, -6.759, 0.844,
    -6.675, -6.139, -2.895, -2.590, -3.680, -5.002, -6.091, -6.393, -6.475, -6.545, -6.600, -6.650,
    0.079, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000,
];
const H2O_F_0: [f64; 35] = [
    22.235080,
    67.803960,
    119.995940,
    183.310087,
    321.225630,
    325.152888,
    336.227764,
    380.197353,
    390.134508,
    437.346667,
    439.150807,
    443.018343,
    448.001085,
    470.888999,
    474.689092,
    488.490108,
    503.568532,
    504.482692,
    547.676440,
    552.020960,
    556.935985,
    620.700807,
    645.766085,
    658.005280,
    752.033113,
    841.051732,
    859.965698,
    899.303175,
    902.611085,
    906.205957,
    916.171582,
    923.112692,
    970.315022,
    987.926764,
    1780.000000,
];
const H2O_B_1: [f64; 35] = [
    0.1079, 0.0011, 0.0007, 2.273, 0.0470, 1.514, 0.0010, 11.67, 0.0045, 0.0632, 0.9098, 0.1920,
    10.41, 0.3254, 1.260, 0.2529, 0.0372, 0.0124, 0.9785, 0.1840, 497.0, 5.015, 0.0067, 0.2732,
    243.4, 0.0134, 0.1325, 0.0547, 0.0386, 0.1836, 8.400, 0.0079, 9.009, 134.6, 17506.0,
];
const H2O_B_2: [f64; 35] = [
    2.144, 8.732, 8.353, 0.668, 6.179, 1.541, 9.825, 1.048, 7.347, 5.048, 3.595, 5.048, 1.405,
    3.597, 2.379, 2.852, 6.731, 6.731, 0.158, 0.158, 0.159, 2.391, 8.633, 7.816, 0.396, 8.177,
    8.055, 7.914, 8.429, 5.110, 1.441, 10.293, 1.919, 0.257, 0.952,
];
const H2O_B_3: [f64; 35] = [
    26.38, 28.58, 29.48, 29.06, 24.04, 28.23, 26.93, 28.11, 21.52, 18.45, 20.07, 15.55, 25.64,
    21.34, 23.20, 25.86, 16.12, 16.12, 26.00, 26.00, 30.86, 24.38, 18.00, 32.10, 30.86, 15.90,
    30.60, 29.85, 28.65, 24.08, 26.73, 29.00, 25.50, 29.85, 196.3,
];
const H2O_B_4: [f64; 35] = [
    0.76, 0.69, 0.70, 0.77, 0.67, 0.64, 0.69, 0.54, 0.63, 0.60, 0.63, 0.60, 0.66, 0.66, 0.65, 0.69,
    0.61, 0.61, 0.70, 0.70, 0.69, 0.71, 0.60, 0.69, 0.68, 0.33, 0.68, 0.68, 0.70, 0.70, 0.70, 0.70,
    0.64, 0.68, 2.00,
];
const H2O_B_5: [f64; 35] = [
    5.087, 4.930, 4.780, 5.022, 4.398, 4.893, 4.740, 5.063, 4.810, 4.230, 4.483, 5.083, 5.028,
    4.506, 4.804, 5.201, 3.980, 4.010, 4.500, 4.500, 4.552, 4.856, 4.000, 4.140, 4.352, 5.760,
    4.090, 4.530, 5.100, 4.700, 5.150, 5.000, 4.940, 4.550, 24.15,
];
const H2O_B_6: [f64; 35] = [
    1.00, 0.82, 0.79, 0.85, 0.54, 0.74, 0.61, 0.89, 0.55, 0.48, 0.52, 0.50, 0.67, 0.65, 0.64, 0.72,
    0.43, 0.45, 1.00, 1.00, 1.00, 0.68, 0.50, 1.00, 0.84, 0.45, 0.84, 0.90, 0.95, 0.53, 0.78, 0.80,
    0.67, 0.90, 5.00,
];
const NAKAGAMI_RICE: [[f64; 17]; 17] = [
    [
        -0.1417, -0.1252, -0.1004, -0.0784, -0.0634, -0.0515, -0.0321, -0.0155, 0.0000, 0.0156,
        0.0323, 0.0518, 0.0639, 0.0791, 0.1016, 0.1271, 0.1441,
    ],
    [
        -0.7676, -0.6811, -0.5497, -0.4312, -0.3504, -0.2856, -0.1790, -0.0870, 0.0000, 0.0878,
        0.1828, 0.2953, 0.3651, 0.4537, 0.5868, 0.7390, 0.8420,
    ],
    [
        -1.3183, -1.1738, -0.9524, -0.7508, -0.6121, -0.5003, -0.3151, -0.1537, 0.0000, 0.1564,
        0.3269, 0.5308, 0.6585, 0.8218, 1.0696, 1.3572, 1.5544,
    ],
    [
        -1.6263, -1.4507, -1.1805, -0.9332, -0.7623, -0.6240, -0.3940, -0.1926, 0.0000, 0.1969,
        0.4127, 0.6722, 0.8355, 1.0453, 1.3660, 1.7417, 2.0014,
    ],
    [
        -1.9963, -1.7847, -1.4573, -1.1557, -0.9462, -0.7760, -0.4916, -0.2410, 0.0000, 0.2478,
        0.5209, 0.8519, 1.0615, 1.3326, 1.7506, 2.2463, 2.5931,
    ],
    [
        -2.4355, -2.1829, -1.7896, -1.4247, -1.1695, -0.9613, -0.6113, -0.3007, 0.0000, 0.3114,
        0.6573, 1.0802, 1.3505, 1.7028, 2.2526, 2.9156, 3.3872,
    ],
    [
        -2.9491, -2.6507, -2.1831, -1.7455, -1.4375, -1.1846, -0.7567, -0.3737, 0.0000, 0.3903,
        0.8281, 1.3698, 1.7198, 2.1808, 2.9119, 3.8143, 4.4714,
    ],
    [
        -3.5384, -3.1902, -2.6407, -2.1218, -1.7535, -1.4495, -0.9307, -0.4619, 0.0000, 0.4874,
        1.0404, 1.7348, 2.1898, 2.7975, 3.7820, 5.0373, 5.9833,
    ],
    [
        -4.1980, -3.7974, -3.1602, -2.5528, -2.1180, -1.7565, -1.1345, -0.5662, 0.0000, 0.6045,
        1.2999, 2.1887, 2.7814, 3.5868, 4.9288, 6.7171, 8.1319,
    ],
    [
        -4.9132, -4.4591, -3.7313, -3.0306, -2.5247, -2.1011, -1.3655, -0.6855, 0.0000, 0.7415,
        1.6078, 2.7374, 3.5059, 4.5714, 6.4060, 8.9732, 11.0973,
    ],
    [
        -5.6559, -5.1494, -4.3315, -3.5366, -2.9578, -2.4699, -1.6150, -0.8154, 0.0000, 0.8935,
        1.9530, 3.3611, 4.3363, 5.7101, 8.1216, 11.5185, 14.2546,
    ],
    [
        -6.3810, -5.8252, -4.9219, -4.0366, -3.3871, -2.8364, -1.8638, -0.9455, 0.0000, 1.0458,
        2.2979, 3.9771, 5.1450, 6.7874, 9.6276, 13.4690, 16.4251,
    ],
    [
        -7.0247, -6.4249, -5.4449, -4.4782, -3.7652, -3.1580, -2.0804, -1.0574, 0.0000, 1.1723,
        2.5755, 4.4471, 5.7363, 7.5266, 10.5553, 14.5401, 17.5511,
    ],
    [
        -7.5229, -6.8862, -5.8424, -4.8090, -4.0446, -3.3927, -2.2344, -1.1347, 0.0000, 1.2535,
        2.7446, 4.7144, 6.0581, 7.9073, 11.0003, 15.0270, 18.0526,
    ],
    [
        -7.8532, -7.1880, -6.0963, -5.0145, -4.2145, -3.5325, -2.3227, -1.1774, 0.0000, 1.2948,
        2.8268, 4.8377, 6.2021, 8.0724, 11.1869, 15.2265, 18.2566,
    ],
    [
        -8.0435, -7.3588, -6.2354, -5.1234, -4.3022, -3.6032, -2.3656, -1.1975, 0.0000, 1.3130,
        2.8619, 4.8888, 6.2610, 8.1388, 11.2607, 15.3047, 18.3361,
    ],
    [
        -8.2238, -7.5154, -6.3565, -5.2137, -4.3726, -3.6584, -2.3979, -1.2121, 0.0000, 1.3255,
        2.8855, 4.9224, 6.2992, 8.1814, 11.3076, 15.3541, 18.3864,
    ],
];
const K_VALUES: [f64; 17] = [
    -40.0, -25.0, -20.0, -18.0, -16.0, -14.0, -12.0, -10.0, -8.0, -6.0, -4.0, -2.0, 0.0, 2.0, 4.0,
    6.0, 20.0,
];
const PERCENTS: [f64; 17] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 85.0, 90.0, 95.0, 98.0,
    99.0,
];
