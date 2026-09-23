use num_complex::Complex64 as C64;
use std::f64::consts::{PI, SQRT_2};

const A_0: f64 = 6370e3;
const A_9000: f64 = 9000e3;
const THIRD: f64 = 1.0 / 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Climate {
    Equatorial = 1,
    ContinentalSubtropical = 2,
    MaritimeSubtropical = 3,
    Desert = 4,
    ContinentalTemperate = 5,
    MaritimeTemperateOverLand = 6,
    MaritimeTemperateOverSea = 7,
}

impl Climate {
    pub const ALL: [Climate; 7] = [
        Climate::Equatorial,
        Climate::ContinentalSubtropical,
        Climate::MaritimeSubtropical,
        Climate::Desert,
        Climate::ContinentalTemperate,
        Climate::MaritimeTemperateOverLand,
        Climate::MaritimeTemperateOverSea,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Climate::Equatorial => "equatorial",
            Climate::ContinentalSubtropical => "continental subtropical",
            Climate::MaritimeSubtropical => "maritime subtropical",
            Climate::Desert => "desert",
            Climate::ContinentalTemperate => "continental temperate",
            Climate::MaritimeTemperateOverLand => "maritime temperate, land",
            Climate::MaritimeTemperateOverSea => "maritime temperate, sea",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    LineOfSight,
    Diffraction,
    Troposcatter,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::LineOfSight => "line of sight",
            Mode::Diffraction => "diffraction",
            Mode::Troposcatter => "troposcatter",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub climate: Climate,
    pub n_0: f64,
    pub vertical: bool,
    pub epsilon: f64,
    pub sigma: f64,
    pub mdvar: i32,
    pub time: f64,
    pub location: f64,
    pub situation: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            climate: Climate::ContinentalTemperate,
            n_0: 301.0,
            vertical: true,
            epsilon: 15.0,
            sigma: 0.005,
            mdvar: 12,
            time: 50.0,
            location: 50.0,
            situation: 50.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Result {
    pub loss_db: f64,
    pub free_space_db: f64,
    pub reference_db: f64,
    pub mode: Mode,
    pub warnings: u32,
    pub delta_h: f64,
    pub d_hzn: [f64; 2],
    pub h_e: [f64; 2],
    pub theta_hzn: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TerminalHeight,
    Frequency,
    Refractivity,
    Ground,
    Percentages,
    EffectiveEarth,
    ShortProfile,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::TerminalHeight => "antenna heights must be 0.5 to 3000 m",
            Error::Frequency => "Longley-Rice covers 20 MHz to 20 GHz",
            Error::Refractivity => "surface refractivity out of range",
            Error::Ground => "ground constants out of range",
            Error::Percentages => "time, location and situation must be between 0 and 100 %",
            Error::EffectiveEarth => "effective earth radius out of range",
            Error::ShortProfile => "the profile needs at least two intervals",
        })
    }
}

struct Pfl<'a> {
    z: &'a [f64],
    xi: f64,
}

impl Pfl<'_> {
    fn np(&self) -> usize {
        self.z.len() - 1
    }
}

fn dim(a: f64, b: f64) -> f64 {
    (a - b).max(0.0)
}

pub fn point_to_point(
    h_tx: f64,
    h_rx: f64,
    ground: &[f64],
    step: f64,
    f_mhz: f64,
    p: &Params,
) -> std::result::Result<Result, Error> {
    if ground.len() < 3 {
        return Err(Error::ShortProfile);
    }
    if !(0.5..=3000.0).contains(&h_tx) || !(0.5..=3000.0).contains(&h_rx) {
        return Err(Error::TerminalHeight);
    }
    if !(20.0..=20000.0).contains(&f_mhz) {
        return Err(Error::Frequency);
    }
    if !(250.0..=400.0).contains(&p.n_0) {
        return Err(Error::Refractivity);
    }
    if p.epsilon < 1.0 || p.sigma <= 0.0 {
        return Err(Error::Ground);
    }
    let pct = |v: f64| v > 0.0 && v < 100.0;
    if !pct(p.time) || !pct(p.location) || !pct(p.situation) {
        return Err(Error::Percentages);
    }
    let mut warnings = 0u32;
    if !(1.0..=1000.0).contains(&h_tx) {
        warnings |= 0x0001;
    }
    if !(1.0..=1000.0).contains(&h_rx) {
        warnings |= 0x0002;
    }
    if !(40.0..=10000.0).contains(&f_mhz) {
        warnings |= 0x0004;
    }
    let pfl = Pfl { z: ground, xi: step };
    let np = pfl.np();
    let p10 = (0.1 * np as f64) as usize;
    let span = &ground[p10..=np - p10];
    let h_sys = span.iter().sum::<f64>() / span.len() as f64;
    let (z_g, gamma_e, n_s) = initialize(f_mhz, h_sys, p.n_0, p.vertical, p.epsilon, p.sigma);
    let h = [h_tx, h_rx];
    let q = quick_pfl(&pfl, gamma_e, h);
    let (a_ref, mode) = longley_rice(
        q.theta_hzn,
        f_mhz,
        z_g,
        q.d_hzn,
        q.h_e,
        gamma_e,
        n_s,
        q.delta_h,
        h,
        q.d,
        &mut warnings,
    )?;
    let a_fs = free_space_loss(q.d, f_mhz);
    let v = variability(
        p.time,
        p.location,
        p.situation,
        q.h_e,
        q.delta_h,
        f_mhz,
        q.d,
        a_ref,
        p.climate as usize,
        p.mdvar,
        &mut warnings,
    );
    Ok(Result {
        loss_db: v + a_fs,
        free_space_db: a_fs,
        reference_db: a_ref,
        mode,
        warnings,
        delta_h: q.delta_h,
        d_hzn: q.d_hzn,
        h_e: q.h_e,
        theta_hzn: q.theta_hzn,
    })
}

fn initialize(
    f_mhz: f64,
    h_sys: f64,
    n_0: f64,
    vertical: bool,
    epsilon: f64,
    sigma: f64,
) -> (C64, f64, f64) {
    let n_s = if h_sys == 0.0 { n_0 } else { n_0 * (-h_sys / 9460.0).exp() };
    let gamma_e = 157e-9 * (1.0 - 0.04665 * (n_s / 179.3).exp());
    let ep_r = C64::new(epsilon, 18000.0 * sigma / f_mhz);
    let mut z_g = (ep_r - 1.0).sqrt();
    if vertical {
        z_g /= ep_r;
    }
    (z_g, gamma_e, n_s)
}

struct Quick {
    theta_hzn: [f64; 2],
    d_hzn: [f64; 2],
    h_e: [f64; 2],
    delta_h: f64,
    d: f64,
}

fn quick_pfl(pfl: &Pfl, gamma_e: f64, h: [f64; 2]) -> Quick {
    let np = pfl.np();
    let d = np as f64 * pfl.xi;
    let a_e = 1.0 / gamma_e;
    let (mut theta_hzn, mut d_hzn) = find_horizons(pfl, a_e, h);
    let d_start = (15.0 * h[0]).min(0.1 * d_hzn[0]);
    let d_end = d - (15.0 * h[1]).min(0.1 * d_hzn[1]);
    let delta_h = compute_delta_h(pfl, d_start, d_end);
    let mut h_e = [0.0; 2];
    if d_hzn[0] + d_hzn[1] > 1.5 * d {
        let (fit_tx, fit_rx) = linear_least_squares_fit(pfl, d_start, d_end);
        h_e[0] = h[0] + dim(pfl.z[0], fit_tx);
        h_e[1] = h[1] + dim(pfl.z[np], fit_rx);
        let horizon =
            |he: f64| (2.0 * he * a_e).sqrt() * (-0.07 * (delta_h / he.max(5.0)).sqrt()).exp();
        for i in 0..2 {
            d_hzn[i] = horizon(h_e[i]);
        }
        let combined = d_hzn[0] + d_hzn[1];
        if combined <= d {
            let q = (d / combined).powi(2);
            for i in 0..2 {
                h_e[i] *= q;
                d_hzn[i] = horizon(h_e[i]);
            }
        }
        for i in 0..2 {
            let q = (2.0 * h_e[i] * a_e).sqrt();
            theta_hzn[i] = (0.65 * delta_h * (q / d_hzn[i] - 1.0) - 2.0 * h_e[i]) / q;
        }
    } else {
        let (fit_tx, _) = linear_least_squares_fit(pfl, d_start, 0.9 * d_hzn[0]);
        h_e[0] = h[0] + dim(pfl.z[0], fit_tx);
        let (_, fit_rx) = linear_least_squares_fit(pfl, d - 0.9 * d_hzn[1], d_end);
        h_e[1] = h[1] + dim(pfl.z[np], fit_rx);
    }
    Quick { theta_hzn, d_hzn, h_e, delta_h, d }
}

fn find_horizons(pfl: &Pfl, a_e: f64, h: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    let np = pfl.np();
    let xi = pfl.xi;
    let d = np as f64 * xi;
    let z_tx = pfl.z[0] + h[0];
    let z_rx = pfl.z[np] + h[1];
    let mut theta = [(z_rx - z_tx) / d - d / (2.0 * a_e), -(z_rx - z_tx) / d - d / (2.0 * a_e)];
    let mut dh = [d, d];
    let mut d_tx = 0.0;
    let mut d_rx = d;
    for i in 1..np {
        d_tx += xi;
        d_rx -= xi;
        let t_tx = (pfl.z[i] - z_tx) / d_tx - d_tx / (2.0 * a_e);
        let t_rx = -(z_rx - pfl.z[i]) / d_rx - d_rx / (2.0 * a_e);
        if t_tx > theta[0] {
            theta[0] = t_tx;
            dh[0] = d_tx;
        }
        if t_rx > theta[1] {
            theta[1] = t_rx;
            dh[1] = d_rx;
        }
    }
    (theta, dh)
}

fn compute_delta_h(pfl: &Pfl, d_start: f64, d_end: f64) -> f64 {
    let np = pfl.np();
    let mut x_start = d_start / pfl.xi;
    let x_end = d_end / pfl.xi;
    if x_end - x_start < 2.0 {
        return 0.0;
    }
    let p10 = ((0.1 * (x_end - x_start + 8.0)) as usize).clamp(4, 25);
    let n = 10 * p10 - 5;
    let p90 = n - p10;
    let np_s = (n - 1) as f64;
    let step = (x_end - x_start) / np_s;
    let mut i = x_start as usize;
    x_start -= (i + 1) as f64;
    let mut s = [0.0f64; 245];
    for sj in s.iter_mut().take(n) {
        while x_start > 0.0 && i + 1 < np {
            x_start -= 1.0;
            i += 1;
        }
        *sj = pfl.z[i + 1] + (pfl.z[i + 1] - pfl.z[i]) * x_start;
        x_start += step;
    }
    let sp = Pfl { z: &s[..n], xi: 1.0 };
    let (mut fit1, fit2) = linear_least_squares_fit(&sp, 0.0, np_s);
    let slope = (fit2 - fit1) / np_s;
    let mut diffs = [0.0f64; 245];
    for j in 0..n {
        diffs[j] = s[j] - fit1;
        fit1 += slope;
    }
    let diffs = &mut diffs[..n];
    diffs.sort_unstable_by(|a, b| b.total_cmp(a));
    let q10 = diffs[p10 - 1];
    let q90 = diffs[p90];
    (q10 - q90) / (1.0 - 0.8 * (-(d_end - d_start) / 50e3).exp())
}

fn linear_least_squares_fit(pfl: &Pfl, d_start: f64, d_end: f64) -> (f64, f64) {
    let np = pfl.np() as i64;
    let npf = np as f64;
    let mut i_start = dim(d_start / pfl.xi, 0.0) as i64;
    let mut i_end = np - dim(npf, d_end / pfl.xi) as i64;
    if i_end <= i_start {
        i_start = dim(i_start as f64, 1.0) as i64;
        i_end = np - dim(npf, i_end as f64 + 1.0) as i64;
    }
    let x_length = (i_end - i_start) as f64;
    let mut mid = -0.5 * x_length;
    let mid_end = i_end as f64 + mid;
    let z = |i: i64| pfl.z[i as usize];
    let mut sum_y = 0.5 * (z(i_start) + z(i_end));
    let mut scaled = 0.5 * (z(i_start) - z(i_end)) * mid;
    let mut k = 2.0;
    while k <= x_length {
        i_start += 1;
        mid += 1.0;
        sum_y += z(i_start);
        scaled += z(i_start) * mid;
        k += 1.0;
    }
    sum_y /= x_length;
    scaled *= 12.0 / ((x_length * x_length + 2.0) * x_length);
    (sum_y - scaled * mid_end, sum_y + scaled * (npf - mid_end))
}

#[allow(clippy::too_many_arguments)]
fn longley_rice(
    theta_hzn: [f64; 2],
    f_mhz: f64,
    z_g: C64,
    d_hzn: [f64; 2],
    h_e: [f64; 2],
    gamma_e: f64,
    n_s: f64,
    delta_h: f64,
    h: [f64; 2],
    d: f64,
    warnings: &mut u32,
) -> std::result::Result<(f64, Mode), Error> {
    let a_e = 1.0 / gamma_e;
    let d_hzn_s = [(2.0 * h_e[0] * a_e).sqrt(), (2.0 * h_e[1] * a_e).sqrt()];
    let d_sml = d_hzn_s[0] + d_hzn_s[1];
    let d_ml = d_hzn[0] + d_hzn[1];
    let theta_los = -(theta_hzn[0] + theta_hzn[1]).max(-d_ml / a_e);
    for i in 0..2 {
        if theta_hzn[i].abs() > 200e-3 {
            *warnings |= 0x0080 << i;
        }
        if d_hzn[i] < 0.1 * d_hzn_s[i] {
            *warnings |= 0x0200 << i;
        }
        if d_hzn[i] > 3.0 * d_hzn_s[i] {
            *warnings |= 0x0800 << i;
        }
    }
    if !(150.0..=400.0).contains(&n_s) {
        return Err(Error::Refractivity);
    }
    if n_s < 250.0 {
        *warnings |= 0x4000;
    }
    if !(4_000_000.0..=13_333_333.0).contains(&a_e) {
        return Err(Error::EffectiveEarth);
    }
    if z_g.re <= z_g.im.abs() {
        return Err(Error::Ground);
    }
    let scale = (a_e * a_e / f_mhz).powf(THIRD);
    let d_3 = d_sml.max(d_ml + 5.0 * scale);
    let d_4 = d_3 + 10.0 * scale;
    let diff = |dist: f64| {
        diffraction_loss(dist, d_hzn, h_e, z_g, a_e, delta_h, h, theta_los, d_sml, f_mhz)
    };
    let a_3 = diff(d_3);
    let a_4 = diff(d_4);
    let m_d = (a_4 - a_3) / (d_4 - d_3);
    let a_d0 = a_3 - m_d * d_3;
    if d < (h_e[0] - h_e[1]).abs() / 200e-3 {
        *warnings |= 0x0020;
    }
    if d < 1e3 {
        *warnings |= 0x0040;
    }
    if d > 1000e3 {
        *warnings |= 0x0008;
    }
    if d > 2000e3 {
        *warnings |= 0x0010;
    }
    let (a_ref, mode) = if d < d_sml {
        let a_sml = d_sml * m_d + a_d0;
        let mut d_0 = 0.04 * f_mhz * h_e[0] * h_e[1];
        let d_1 = if a_d0 >= 0.0 {
            d_0 = d_0.min(0.5 * d_ml);
            d_0 + 0.25 * (d_ml - d_0)
        } else {
            (-a_d0 / m_d).max(0.25 * d_ml)
        };
        let los = |dist: f64| line_of_sight_loss(dist, h_e, z_g, delta_h, m_d, a_d0, d_sml, f_mhz);
        let a_1 = los(d_1);
        let mut flag = false;
        let mut k_1 = 0.0;
        let mut k_2 = 0.0;
        if d_0 < d_1 {
            let a_0 = los(d_0);
            let q = (d_sml / d_0).ln();
            k_2 = (((d_sml - d_0) * (a_1 - a_0) - (d_1 - d_0) * (a_sml - a_0))
                / ((d_sml - d_0) * (d_1 / d_0).ln() - (d_1 - d_0) * q))
                .max(0.0);
            flag = a_d0 > 0.0 || k_2 > 0.0;
            if flag {
                k_1 = (a_sml - a_0 - k_2 * q) / (d_sml - d_0);
                if k_1 < 0.0 {
                    k_1 = 0.0;
                    k_2 = dim(a_sml, a_0) / q;
                    if k_2 == 0.0 {
                        k_1 = m_d;
                    }
                }
            }
        }
        if !flag {
            k_1 = dim(a_sml, a_1) / (d_sml - d_1);
            k_2 = 0.0;
            if k_1 == 0.0 {
                k_1 = m_d;
            }
        }
        let a_o = a_sml - k_1 * d_sml - k_2 * d_sml.ln();
        (a_o + k_1 * d + k_2 * d.ln(), Mode::LineOfSight)
    } else {
        let d_5 = d_ml + 200e3;
        let d_6 = d_ml + 400e3;
        let mut h0 = -1.0;
        let a_6 =
            troposcatter_loss(d_6, theta_hzn, d_hzn, h_e, a_e, n_s, f_mhz, theta_los, &mut h0);
        let a_5 =
            troposcatter_loss(d_5, theta_hzn, d_hzn, h_e, a_e, n_s, f_mhz, theta_los, &mut h0);
        let (m_s, a_s0, d_x) = if a_5 < 1000.0 {
            let m_s = (a_6 - a_5) / 200e3;
            let d_x = d_sml
                .max(d_ml + 1.088 * scale * f_mhz.ln())
                .max((a_5 - a_d0 - m_s * d_5) / (m_d - m_s));
            (m_s, (m_d - m_s) * d_x + a_d0, d_x)
        } else {
            (m_d, a_d0, 10e6)
        };
        if d > d_x {
            (m_s * d + a_s0, Mode::Troposcatter)
        } else {
            (m_d * d + a_d0, Mode::Diffraction)
        }
    };
    Ok((a_ref.max(0.0), mode))
}

#[allow(clippy::too_many_arguments)]
fn diffraction_loss(
    d: f64,
    d_hzn: [f64; 2],
    h_e: [f64; 2],
    z_g: C64,
    a_e: f64,
    delta_h: f64,
    h: [f64; 2],
    theta_los: f64,
    d_sml: f64,
    f_mhz: f64,
) -> f64 {
    let a_k = knife_edge_diffraction(d, f_mhz, a_e, theta_los, d_hzn);
    let a_se = smooth_earth_diffraction(d, f_mhz, a_e, theta_los, d_hzn, h_e, z_g);
    let sigma_h_d = sigma_h(terrain_roughness(d_sml, delta_h));
    let a_fo = (5.0 * (1.0 + 1e-5 * h[0] * h[1] * f_mhz * sigma_h_d).log10()).min(15.0);
    let delta_h_d = terrain_roughness(d, delta_h);
    let q = h[0] * h[1];
    let qk = h_e[0] * h_e[1] - q;
    let q = q + 10.0;
    let term1 = (1.0 + qk / q).sqrt();
    let d_ml = d_hzn[0] + d_hzn[1];
    let q = (term1 + (-theta_los * a_e + d_ml) / d) * (delta_h_d * f_mhz / 47.7).min(6283.2);
    let w = 25.1 / (25.1 + q.sqrt());
    w * a_se + (1.0 - w) * a_k + a_fo
}

#[allow(clippy::too_many_arguments)]
fn line_of_sight_loss(
    d: f64,
    h_e: [f64; 2],
    z_g: C64,
    delta_h: f64,
    m_d: f64,
    a_d0: f64,
    d_sml: f64,
    f_mhz: f64,
) -> f64 {
    let sigma_h_d = sigma_h(terrain_roughness(d, delta_h));
    let wn = f_mhz / 47.7;
    let sin_psi = (h_e[0] + h_e[1]) / (d * d + (h_e[0] + h_e[1]).powi(2)).sqrt();
    let mut r_e = (sin_psi - z_g) / (sin_psi + z_g) * (-(wn * sigma_h_d * sin_psi).min(10.0)).exp();
    let q = r_e.norm_sqr();
    if q < 0.25 || q < sin_psi {
        r_e *= (sin_psi / q).sqrt();
    }
    let mut delta_phi = wn * 2.0 * h_e[0] * h_e[1] / d;
    if delta_phi > PI / 2.0 {
        delta_phi = PI - (PI / 2.0).powi(2) / delta_phi;
    }
    let rr = C64::new(delta_phi.cos(), -delta_phi.sin()) + r_e;
    let a_t = -10.0 * rr.norm_sqr().log10();
    let a_d = m_d * d + a_d0;
    let w = 1.0 / (1.0 + f_mhz * delta_h / d_sml.max(10e3));
    w * a_t + (1.0 - w) * a_d
}

fn smooth_earth_diffraction(
    d: f64,
    f_mhz: f64,
    a_e: f64,
    theta_los: f64,
    d_hzn: [f64; 2],
    h_e: [f64; 2],
    z_g: C64,
) -> f64 {
    let theta_nlos = d / a_e - theta_los;
    let d_ml = d_hzn[0] + d_hzn[1];
    let a = [
        (d - d_ml) / (d / a_e - theta_los),
        0.5 * d_hzn[0].powi(2) / h_e[0],
        0.5 * d_hzn[1].powi(2) / h_e[1],
    ];
    let d_km = [a[0] * theta_nlos / 1000.0, d_hzn[0] / 1000.0, d_hzn[1] / 1000.0];
    let mut k = [0.0; 3];
    let mut b_0 = [0.0; 3];
    let mut c_0 = [0.0; 3];
    for i in 0..3 {
        c_0[i] = ((4.0 / 3.0) * A_0 / a[i]).powf(THIRD);
        k[i] = 0.017778 * c_0[i] * f_mhz.powf(-THIRD) / z_g.norm();
        b_0[i] = 1.607 - k[i];
    }
    let x1 = b_0[1] * c_0[1].powi(2) * f_mhz.powf(THIRD) * d_km[1];
    let x2 = b_0[2] * c_0[2].powi(2) * f_mhz.powf(THIRD) * d_km[2];
    let x0 = b_0[0] * c_0[0].powi(2) * f_mhz.powf(THIRD) * d_km[0] + x1 + x2;
    let f1 = height_function(x1, k[1]);
    let f2 = height_function(x2, k[2]);
    let g = 0.05751 * x0 - 10.0 * x0.log10();
    g - f1 - f2 - 20.0
}

fn height_function(x_km: f64, k: f64) -> f64 {
    if x_km < 200.0 {
        let w = -k.ln();
        if k < 1e-5 || x_km * w.powi(3) > 5495.0 {
            if x_km > 1.0 { 17.372 * x_km.ln() - 117.0 } else { -117.0 }
        } else {
            2.5e-5 * x_km * x_km / k - 8.686 * w - 15.0
        }
    } else {
        let r = 0.05751 * x_km - 4.343 * x_km.ln();
        if x_km < 2000.0 {
            let w = 0.0134 * x_km * (-0.005 * x_km).exp();
            (1.0 - w) * r + w * (17.372 * x_km.ln() - 117.0)
        } else {
            r
        }
    }
}

fn knife_edge_diffraction(d: f64, f_mhz: f64, a_e: f64, theta_los: f64, d_hzn: [f64; 2]) -> f64 {
    let d_ml = d_hzn[0] + d_hzn[1];
    let theta_nlos = d / a_e - theta_los;
    let d_nlos = d - d_ml;
    let v = |dh: f64| {
        0.0795775 * (f_mhz / 47.7) * theta_nlos * theta_nlos * dh * d_nlos / (d_nlos + dh)
    };
    fresnel_integral(v(d_hzn[0])) + fresnel_integral(v(d_hzn[1]))
}

fn fresnel_integral(v2: f64) -> f64 {
    if v2 < 5.76 { 6.02 + 9.11 * v2.sqrt() - 1.27 * v2 } else { 12.953 + 10.0 * v2.log10() }
}

fn terrain_roughness(d: f64, delta_h: f64) -> f64 {
    delta_h * (1.0 - 0.8 * (-d / 50e3).exp())
}

fn sigma_h(delta_h: f64) -> f64 {
    0.78 * delta_h * (-0.5 * delta_h.powf(0.25)).exp()
}

fn f_function(td: f64) -> f64 {
    let (a, b, c) = if td <= 10e3 {
        (133.4, 0.332e-3, -10.0)
    } else if td <= 70e3 {
        (104.6, 0.212e-3, -2.5)
    } else {
        (71.8, 0.157e-3, 5.0)
    };
    a + b * td + c * td.log10()
}

fn h0_curve(j: usize, r: f64) -> f64 {
    const A: [f64; 5] = [25.0, 80.0, 177.0, 395.0, 705.0];
    const B: [f64; 5] = [24.0, 45.0, 68.0, 80.0, 105.0];
    10.0 * (1.0 + A[j] * (1.0 / r).powi(4) + B[j] * (1.0 / r).powi(2)).log10()
}

fn h0_function(r: f64, eta_s: f64) -> f64 {
    let eta_s = eta_s.clamp(1.0, 5.0);
    let i = eta_s as usize;
    let q = eta_s - i as f64;
    let mut result = h0_curve(i - 1, r);
    if q != 0.0 {
        result = (1.0 - q) * result + q * h0_curve(i, r);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn troposcatter_loss(
    d: f64,
    theta_hzn: [f64; 2],
    d_hzn: [f64; 2],
    h_e: [f64; 2],
    a_e: f64,
    n_s: f64,
    f_mhz: f64,
    theta_los: f64,
    h0: &mut f64,
) -> f64 {
    let wn = f_mhz / 47.7;
    let h_0 = if *h0 > 15.0 {
        *h0
    } else {
        let mut ad = d_hzn[0] - d_hzn[1];
        let mut rr = h_e[1] / h_e[0];
        if ad < 0.0 {
            ad = -ad;
            rr = 1.0 / rr;
        }
        let theta = theta_hzn[0] + theta_hzn[1] + d / a_e;
        let r_1 = 2.0 * wn * theta * h_e[0];
        let r_2 = 2.0 * wn * theta * h_e[1];
        if r_1 < 0.2 && r_2 < 0.2 {
            return 1001.0;
        }
        let s = (d - ad) / (d + ad);
        let q = (rr / s).clamp(0.1, 10.0);
        let s = s.max(0.1);
        let h_0_m = (d - ad) * (d + ad) * theta * 0.25 / d;
        let eta_s = (h_0_m / 1.7556e3)
            * (1.0
                + (0.031 - n_s * 2.32e-3 + n_s * n_s * 5.67e-6)
                    * (-(h_0_m / 8.0e3).min(1.7).powi(6)).exp());
        let h_00 = (h0_function(r_1, eta_s) + h0_function(r_2, eta_s)) / 2.0;
        let delta = h_00.min(6.0 * (0.6 - eta_s.max(1.0).log10()) * s.log10() * q.log10());
        let mut h_0 = (h_00 + delta).max(0.0);
        if eta_s < 1.0 {
            h_0 = eta_s * h_0
                + (1.0 - eta_s)
                    * 10.0
                    * (((1.0 + SQRT_2 / r_1) * (1.0 + SQRT_2 / r_2)).powi(2) * (r_1 + r_2)
                        / (r_1 + r_2 + 2.0 * SQRT_2))
                        .log10();
        }
        if h_0 > 15.0 && *h0 >= 0.0 { *h0 } else { h_0 }
    };
    *h0 = h_0;
    let th = d / a_e - theta_los;
    f_function(th * d) + 10.0 * (wn * 47.7 * th.powi(4)).log10()
        - 0.1 * (n_s - 301.0) * (-th * d / 40e3).exp()
        + h_0
}

pub fn free_space_loss(d: f64, f_mhz: f64) -> f64 {
    32.45 + 20.0 * f_mhz.log10() + 20.0 * (d / 1000.0).log10()
}

fn curve(c1: f64, c2: f64, x1: f64, x2: f64, x3: f64, d_e: f64) -> f64 {
    (c1 + c2 / (1.0 + ((d_e - x2) / x3).powi(2))) * (d_e / x1).powi(2) / (1.0 + (d_e / x1).powi(2))
}

fn inverse_ccdf(q: f64) -> f64 {
    let x = if q > 0.5 { 1.0 - q } else { q };
    let t = (-2.0 * x.ln()).sqrt();
    let zeta = ((0.010328 * t + 0.802853) * t + 2.515516)
        / (((0.001308 * t + 0.189269) * t + 1.432788) * t + 1.0);
    if q > 0.5 { -(t - zeta) } else { t - zeta }
}

#[allow(clippy::too_many_arguments)]
fn variability(
    time: f64,
    location: f64,
    situation: f64,
    h_e: [f64; 2],
    delta_h: f64,
    f_mhz: f64,
    d: f64,
    a_ref: f64,
    climate: usize,
    mdvar: i32,
    warnings: &mut u32,
) -> f64 {
    const ALL_YEAR: [[f64; 7]; 5] = [
        [-9.67, -0.62, 1.26, -9.21, -0.62, -0.39, 3.15],
        [12.7, 9.19, 15.5, 9.05, 9.19, 2.86, 857.9],
        [144.9e3, 228.9e3, 262.6e3, 84.1e3, 228.9e3, 141.7e3, 2222.0e3],
        [190.3e3, 205.2e3, 185.2e3, 101.1e3, 205.2e3, 315.9e3, 164.8e3],
        [133.8e3, 143.6e3, 99.8e3, 98.6e3, 143.6e3, 167.4e3, 116.3e3],
    ];
    const BSM1: [f64; 7] = [2.13, 2.66, 6.11, 1.98, 2.68, 6.86, 8.51];
    const BSM2: [f64; 7] = [159.5, 7.67, 6.65, 13.11, 7.16, 10.38, 169.8];
    const XSM1: [f64; 7] = [762.2e3, 100.4e3, 138.2e3, 139.1e3, 93.7e3, 187.8e3, 609.8e3];
    const XSM2: [f64; 7] = [123.6e3, 172.5e3, 242.2e3, 132.7e3, 186.8e3, 169.6e3, 119.9e3];
    const XSM3: [f64; 7] = [94.5e3, 136.4e3, 178.6e3, 193.5e3, 133.5e3, 108.9e3, 106.6e3];
    const BSP1: [f64; 7] = [2.11, 6.87, 10.08, 3.68, 4.75, 8.58, 8.43];
    const BSP2: [f64; 7] = [102.3, 15.53, 9.60, 159.3, 8.12, 13.97, 8.19];
    const XSP1: [f64; 7] = [636.9e3, 138.7e3, 165.3e3, 464.4e3, 93.2e3, 216.0e3, 136.2e3];
    const XSP2: [f64; 7] = [134.8e3, 143.7e3, 225.7e3, 93.1e3, 135.9e3, 152.0e3, 188.5e3];
    const XSP3: [f64; 7] = [95.6e3, 98.6e3, 129.7e3, 94.2e3, 113.4e3, 122.7e3, 122.9e3];
    const C_D: [f64; 7] = [1.224, 0.801, 1.380, 1.000, 1.224, 1.518, 1.518];
    const Z_D: [f64; 7] = [1.282, 2.161, 1.282, 20.0, 1.282, 1.282, 1.282];
    const BFM1: [f64; 7] = [1.0, 1.0, 1.0, 1.0, 0.92, 1.0, 1.0];
    const BFM2: [f64; 7] = [0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0];
    const BFM3: [f64; 7] = [0.0, 0.0, 0.0, 0.0, 1.77, 0.0, 0.0];
    const BFP1: [f64; 7] = [1.0, 0.93, 1.0, 0.93, 0.93, 1.0, 1.0];
    const BFP2: [f64; 7] = [0.0, 0.31, 0.0, 0.19, 0.31, 0.0, 0.0];
    const BFP3: [f64; 7] = [0.0, 2.00, 0.0, 1.79, 2.00, 0.0, 0.0];

    let mut z_t = inverse_ccdf(time / 100.0);
    let mut z_l = inverse_ccdf(location / 100.0);
    let z_s = inverse_ccdf(situation / 100.0);
    let c = climate - 1;
    let wn = f_mhz / 47.7;
    let d_ex = (2.0 * A_9000 * h_e[0]).sqrt()
        + (2.0 * A_9000 * h_e[1]).sqrt()
        + (575.7e12 / wn).powf(THIRD);
    let d_e = if d < d_ex { 130e3 * d / d_ex } else { 130e3 + d - d_ex };
    let mut mdvar = mdvar;
    let plus20 = mdvar >= 20;
    if plus20 {
        mdvar -= 20;
    }
    let sigma_s = if plus20 { 0.0 } else { 5.0 + 3.0 * (-d_e / 100e3).exp() };
    let plus10 = mdvar >= 10;
    if plus10 {
        mdvar -= 10;
    }
    let v_med =
        curve(ALL_YEAR[0][c], ALL_YEAR[1][c], ALL_YEAR[2][c], ALL_YEAR[3][c], ALL_YEAR[4][c], d_e);
    match mdvar {
        0 => {
            z_t = z_s;
            z_l = z_s;
        }
        1 => z_l = z_s,
        2 => z_l = z_t,
        _ => {}
    }
    if z_t.abs() > 3.10 || z_l.abs() > 3.10 || z_s.abs() > 3.10 {
        *warnings |= 0x2000;
    }
    let sigma_l = if plus10 {
        0.0
    } else {
        let dh = terrain_roughness(d, delta_h);
        10.0 * wn * dh / (wn * dh + 13.0)
    };
    let y_l = sigma_l * z_l;
    let q = (0.133 * wn).ln();
    let g_minus = BFM1[c] + BFM2[c] / ((BFM3[c] * q).powi(2) + 1.0);
    let g_plus = BFP1[c] + BFP2[c] / ((BFP3[c] * q).powi(2) + 1.0);
    let sigma_t_minus = curve(BSM1[c], BSM2[c], XSM1[c], XSM2[c], XSM3[c], d_e) * g_minus;
    let sigma_t_plus = curve(BSP1[c], BSP2[c], XSP1[c], XSP2[c], XSP3[c], d_e) * g_plus;
    let sigma_td = C_D[c] * sigma_t_plus;
    let tgtd = (sigma_t_plus - sigma_td) * Z_D[c];
    let sigma_t = if z_t < 0.0 {
        sigma_t_minus
    } else if z_t <= Z_D[c] {
        sigma_t_plus
    } else {
        sigma_td + tgtd / z_t
    };
    let y_t = sigma_t * z_t;
    let y_s_temp =
        sigma_s * sigma_s + y_t * y_t / (7.8 + z_s * z_s) + y_l * y_l / (24.0 + z_s * z_s);
    let (y_r, y_s) = match mdvar {
        0 => (0.0, (sigma_t * sigma_t + sigma_l * sigma_l + y_s_temp).sqrt() * z_s),
        1 => (y_t, (sigma_l * sigma_l + y_s_temp).sqrt() * z_s),
        2 => ((sigma_t * sigma_t + sigma_l * sigma_l).sqrt() * z_t, y_s_temp.sqrt() * z_s),
        _ => (y_t + y_l, y_s_temp.sqrt() * z_s),
    };
    let mut result = a_ref - v_med - y_r - y_s;
    if result < 0.0 {
        result = result * (29.0 - result) / (29.0 - 10.0 * result);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_ccdf_is_symmetric_about_the_median() {
        assert!(inverse_ccdf(0.5).abs() < 1e-3);
        assert!((inverse_ccdf(0.1) - 1.2816).abs() < 5e-4);
        assert!((inverse_ccdf(0.9) + 1.2816).abs() < 5e-4);
    }
}
