#![allow(clippy::too_many_arguments)]

use std::f64::consts::PI;

const A1: [f64; 44] = [
    0.975, 2.529, 6.193, 14.32, 31.24, 64.29, 124.6, 227.3, 389.7, 627.1, 945.3, 543.4, 1331.8,
    1746.6, 2120.1, 2363.7, 1442.1, 2379.9, 2090.7, 2103.4, 2438.0, 2479.5, 2275.9, 1915.4, 1503.0,
    1490.2, 1078.0, 728.7, 461.3, 274.0, 153.0, 80.4, 39.8, 18.56, 8.172, 3.397, 1.334, 940.3,
    67.4, 637.7, 237.4, 98.1, 572.3, 183.1,
];
const A2: [f64; 44] = [
    9.651, 8.653, 7.709, 6.819, 5.983, 5.201, 4.474, 3.8, 3.182, 2.618, 2.109, 0.014, 1.654, 1.255,
    0.91, 0.621, 0.083, 0.387, 0.207, 0.207, 0.386, 0.621, 0.91, 1.255, 0.083, 1.654, 2.108, 2.617,
    3.181, 3.8, 4.473, 5.2, 5.982, 6.818, 7.708, 8.652, 9.65, 0.01, 0.048, 0.044, 0.049, 0.145,
    0.141, 0.145,
];
const A3: [f64; 44] = [
    6.69, 7.17, 7.64, 8.11, 8.58, 9.06, 9.55, 9.96, 10.37, 10.89, 11.34, 17.03, 11.89, 12.23,
    12.62, 12.95, 14.91, 13.53, 14.08, 14.15, 13.39, 12.92, 12.63, 12.17, 15.13, 11.74, 11.34,
    10.88, 10.38, 9.96, 9.55, 9.06, 8.58, 8.11, 7.64, 7.17, 6.69, 16.64, 16.4, 16.4, 16.0, 16.0,
    16.2, 14.7,
];
const A4: [f64; 44] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];
const A5: [f64; 44] = [
    2.566, 2.246, 1.947, 1.667, 1.388, 1.349, 2.227, 3.17, 3.558, 2.56, -1.172, 3.525, -2.378,
    -3.545, -5.416, -1.932, 6.768, -6.561, 6.957, -6.395, 6.342, 1.014, 5.014, 3.029, -4.499,
    1.856, 0.658, -3.036, -3.968, -3.528, -2.548, -1.66, -1.68, -1.956, -2.216, -2.492, -2.773,
    -0.439, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];
const A6: [f64; 44] = [
    6.85, 6.8, 6.729, 6.64, 6.526, 6.206, 5.085, 3.75, 2.654, 2.952, 6.135, -0.978, 6.547, 6.451,
    6.056, 0.436, -1.273, 2.309, -0.776, 0.699, -2.825, -0.584, -6.619, -6.759, 0.844, -6.675,
    -6.139, -2.895, -2.59, -3.68, -5.002, -6.091, -6.393, -6.475, -6.545, -6.6, -6.65, 0.079, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0,
];
const B1: [f64; 35] = [
    0.1079, 0.0011, 0.0007, 2.273, 0.047, 1.514, 0.001, 11.67, 0.0045, 0.0632, 0.9098, 0.192,
    10.41, 0.3254, 1.26, 0.2529, 0.0372, 0.0124, 0.9785, 0.184, 497.0, 5.015, 0.0067, 0.2732,
    243.4, 0.0134, 0.1325, 0.0547, 0.0386, 0.1836, 8.4, 0.0079, 9.009, 134.6, 17506.0,
];
const B2: [f64; 35] = [
    2.144, 8.732, 8.353, 0.668, 6.179, 1.541, 9.825, 1.048, 7.347, 5.048, 3.595, 5.048, 1.405,
    3.597, 2.379, 2.852, 6.731, 6.731, 0.158, 0.158, 0.159, 2.391, 8.633, 7.816, 0.396, 8.177,
    8.055, 7.914, 8.429, 5.11, 1.441, 10.293, 1.919, 0.257, 0.952,
];
const B3: [f64; 35] = [
    26.38, 28.58, 29.48, 29.06, 24.04, 28.23, 26.93, 28.11, 21.52, 18.45, 20.07, 15.55, 25.64,
    21.34, 23.2, 25.86, 16.12, 16.12, 26.0, 26.0, 30.86, 24.38, 18.0, 32.1, 30.86, 15.9, 30.6,
    29.85, 28.65, 24.08, 26.73, 29.0, 25.5, 29.85, 196.3,
];
const B4: [f64; 35] = [
    0.76, 0.69, 0.7, 0.77, 0.67, 0.64, 0.69, 0.54, 0.63, 0.6, 0.63, 0.6, 0.66, 0.66, 0.65, 0.69,
    0.61, 0.61, 0.7, 0.7, 0.69, 0.71, 0.6, 0.69, 0.68, 0.33, 0.68, 0.68, 0.7, 0.7, 0.7, 0.7, 0.64,
    0.68, 2.0,
];
const B5: [f64; 35] = [
    5.087, 4.93, 4.78, 5.022, 4.398, 4.893, 4.74, 5.063, 4.81, 4.23, 4.483, 5.083, 5.028, 4.506,
    4.804, 5.201, 3.98, 4.01, 4.5, 4.5, 4.552, 4.856, 4.0, 4.14, 4.352, 5.76, 4.09, 4.53, 5.1, 4.7,
    5.15, 5.0, 4.94, 4.55, 24.15,
];
const B6: [f64; 35] = [
    1.0, 0.82, 0.79, 0.85, 0.54, 0.74, 0.61, 0.89, 0.55, 0.48, 0.52, 0.5, 0.67, 0.65, 0.64, 0.72,
    0.43, 0.45, 1.0, 1.0, 1.0, 0.68, 0.5, 1.0, 0.84, 0.45, 0.84, 0.9, 0.95, 0.53, 0.78, 0.8, 0.67,
    0.9, 5.0,
];
const FO: [f64; 44] = [
    50.474214, 50.987745, 51.50336, 52.021429, 52.542418, 53.066934, 53.595775, 54.130025,
    54.67118, 55.221384, 55.783815, 56.264774, 56.363399, 56.968211, 57.612486, 58.323877,
    58.446588, 59.164204, 59.590983, 60.306056, 60.434778, 61.150562, 61.800158, 62.41122,
    62.486253, 62.997984, 63.568526, 64.127775, 64.67891, 65.224078, 65.764779, 66.302096,
    66.836834, 67.369601, 67.900868, 68.431006, 68.960312, 118.750334, 368.498246, 424.76302,
    487.249273, 715.392902, 773.83949, 834.145546,
];
const FW: [f64; 35] = [
    22.23508, 67.80396, 119.99594, 183.310087, 321.22563, 325.152888, 336.227764, 380.197353,
    390.134508, 437.346667, 439.150807, 443.018343, 448.001085, 470.888999, 474.689092, 488.490108,
    503.568532, 504.482692, 547.67644, 552.02096, 556.935985, 620.700807, 645.766085, 658.00528,
    752.033113, 841.051732, 859.965698, 899.303175, 902.611085, 906.205957, 916.171582, 923.112692,
    970.315022, 987.926764, 1780.0,
];

fn p676_gaseous(f: f64, press: f64, rho: f64, t: f64) -> (f64, f64) {
    let theta = 300.0 / t;
    let e = rho * t / 216.7;
    let mut oxygen = 0.0;
    for i in 0..FO.len() {
        let s = A1[i] * 1e-7 * press * theta.powi(3) * (A2[i] * (1.0 - theta)).exp();
        let mut df = A3[i] * 1e-4 * (press * theta.powf(0.8 - A4[i]) + 1.1 * e * theta);
        df = (df * df + 2.25e-6).sqrt();
        let delta = (A5[i] + A6[i] * theta) * 1e-4 * (press + e) * theta.powf(0.8);
        let fi = FO[i];
        let shape = f / fi
            * ((df - delta * (fi - f)) / ((fi - f).powi(2) + df * df)
                + (df - delta * (fi + f)) / ((fi + f).powi(2) + df * df));
        oxygen += s * shape;
    }
    let d = 5.6e-4 * (press + e) * theta.powf(0.8);
    let ndf = f
        * press
        * theta
        * theta
        * (6.14e-5 / (d * (1.0 + (f / d).powi(2)))
            + 1.4e-12 * press * theta.powf(1.5) / (1.0 + 1.9e-5 * f.powf(1.5)));
    let g_o = 0.182 * f * (oxygen + ndf);
    let mut water = 0.0;
    for i in 0..FW.len() {
        let s = B1[i] * 1e-1 * e * theta.powf(3.5) * (B2[i] * (1.0 - theta)).exp();
        let mut df = B3[i] * 1e-4 * (press * theta.powf(B4[i]) + B5[i] * e * theta.powf(B6[i]));
        let fi = FW[i];
        df = 0.535 * df + (0.217 * df * df + 2.1316e-12 * fi * fi / theta).sqrt();
        let shape =
            f / fi * (df / ((fi - f).powi(2) + df * df) + df / ((fi + f).powi(2) + df * df));
        water += s * shape;
    }
    (g_o, 0.182 * f * water)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Coastal = 1,
    Inland = 2,
    Sea = 3,
}

pub struct Input<'a> {
    pub f_ghz: f64,
    pub p: f64,
    pub d: &'a [f64],
    pub h: &'a [f64],
    pub g: &'a [f64],
    pub zone: &'a [Zone],
    pub htg: f64,
    pub hrg: f64,
    pub mid_lat: f64,
    pub gt: f64,
    pub gr: f64,
    pub vertical: bool,
    pub dct: f64,
    pub dcr: f64,
    pub press: f64,
    pub temp: f64,
    pub dn: f64,
    pub n0: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Output {
    pub lb: f64,
    pub lbfsg: f64,
    pub lb0p: f64,
    pub ldp: f64,
    pub lbs: f64,
    pub lba: f64,
    pub transhorizon: bool,
}

fn intervals(mask: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, &m) in mask.iter().enumerate() {
        match (m, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, mask.len() - 1));
    }
    out
}

fn section_length(d: &[f64], (a, b): (usize, usize)) -> f64 {
    let last = d[d.len() - 1];
    let mut delta = 0.0;
    if d[b] < last {
        delta += (d[b + 1] - d[b]) / 2.0;
    }
    if d[a] > 0.0 {
        delta += (d[a] - d[a - 1]) / 2.0;
    }
    d[b] - d[a] + delta
}

pub fn path_fraction(d: &[f64], zone: &[Zone], want: Zone) -> f64 {
    let mask: Vec<bool> = zone.iter().map(|z| *z == want).collect();
    let total: f64 = intervals(&mask).into_iter().map(|iv| section_length(d, iv)).sum();
    total / (d[d.len() - 1] - d[0])
}

pub fn longest_section(d: &[f64], mask: &[bool]) -> f64 {
    intervals(mask).into_iter().map(|iv| section_length(d, iv)).fold(0.0, f64::max)
}

fn inv_cum_norm(x: f64) -> f64 {
    let x = x.max(1e-6);
    let tx = (-2.0 * x.ln()).sqrt();
    let ksi = ((0.010328 * tx + 0.802853) * tx + 2.515516698)
        / (((0.001308 * tx + 0.189269) * tx + 1.432788) * tx + 1.0);
    ksi - tx
}

pub fn beta0(phi: f64, dtm: f64, dlm: f64) -> f64 {
    let tau = 1.0 - (-(4.12e-4 * dlm.powf(2.41))).exp();
    let mu1 = (10f64.powf(-dtm / (16.0 - 6.6 * tau)) + 10f64.powf(-5.0 * (0.496 + 0.354 * tau)))
        .powf(0.2)
        .min(1.0);
    if phi.abs() <= 70.0 {
        let mu4 = 10f64.powf((-0.935 + 0.0176 * phi.abs()) * mu1.log10());
        10f64.powf(-0.015 * phi.abs() + 1.67) * mu1 * mu4
    } else {
        let mu4 = 10f64.powf(0.3 * mu1.log10());
        4.17 * mu1 * mu4
    }
}

fn earth_radius(dn: f64) -> (f64, f64) {
    (6371.0 * 157.0 / (157.0 - dn), 6371.0 * 3.0)
}

pub struct Smooth {
    pub hstd: f64,
    pub hsrd: f64,
    pub hte: f64,
    pub hre: f64,
    pub hm: f64,
    pub dlt: f64,
    pub dlr: f64,
    pub theta_t: f64,
    pub theta_r: f64,
    pub theta: f64,
    pub transhorizon: bool,
}

pub fn smooth_earth_heights(d: &[f64], h: &[f64], htg: f64, hrg: f64, ae: f64, f: f64) -> Smooth {
    let n = d.len();
    let dtot = d[n - 1];
    let hts = h[0] + htg;
    let hrs = h[n - 1] + hrg;
    let (mut v1, mut v2) = (0.0, 0.0);
    for i in 1..n {
        v1 += (d[i] - d[i - 1]) * (h[i] + h[i - 1]);
        v2 += (d[i] - d[i - 1])
            * (h[i] * (2.0 * d[i] + d[i - 1]) + h[i - 1] * (d[i] + 2.0 * d[i - 1]));
    }
    let mut hst = (2.0 * v1 * dtot - v2) / (dtot * dtot);
    let mut hsr = (v2 - v1 * dtot) / (dtot * dtot);
    let hh = |i: usize| h[i] - (hts * (dtot - d[i]) + hrs * d[i]) / dtot;
    let inner = 1..n - 1;
    let hobs = inner.clone().map(hh).fold(f64::NEG_INFINITY, f64::max);
    let alpha_obt = inner.clone().map(|i| hh(i) / d[i]).fold(f64::NEG_INFINITY, f64::max);
    let alpha_obr = inner.clone().map(|i| hh(i) / (dtot - d[i])).fold(f64::NEG_INFINITY, f64::max);
    let gt = alpha_obt / (alpha_obt + alpha_obr);
    let gr = alpha_obr / (alpha_obt + alpha_obr);
    let (hstp, hsrp) = if hobs <= 0.0 { (hst, hsr) } else { (hst - hobs * gt, hsr - hobs * gr) };
    let hstd = if hstp >= h[0] { h[0] } else { hstp };
    let hsrd = if hsrp > h[n - 1] { h[n - 1] } else { hsrp };

    let th_t = |i: usize| 1000.0 * ((h[i] - hts) / (1000.0 * d[i]) - d[i] / (2.0 * ae)).atan();
    let theta_td = 1000.0 * ((hrs - hts) / (1000.0 * dtot) - dtot / (2.0 * ae)).atan();
    let theta_rd = 1000.0 * ((hts - hrs) / (1000.0 * dtot) - dtot / (2.0 * ae)).atan();
    let max_t = inner.clone().map(th_t).fold(f64::NEG_INFINITY, f64::max);
    let mut theta_t = max_t.max(theta_td);
    let transhorizon = theta_t > theta_td;
    let mut lt = inner.clone().find(|&i| th_t(i) == max_t).unwrap_or(1);
    let mut dlt = d[lt];
    let th_r = |i: usize| {
        1000.0 * ((h[i] - hrs) / (1000.0 * (dtot - d[i])) - (dtot - d[i]) / (2.0 * ae)).atan()
    };
    let max_r = inner.clone().map(th_r).fold(f64::NEG_INFINITY, f64::max);
    let mut theta_r = max_r.max(theta_rd);
    let mut lr = inner.clone().rfind(|&i| th_r(i) == max_r).unwrap_or(n - 2);
    let mut dlr = dtot - d[lr];
    if !transhorizon {
        theta_t = theta_td;
        theta_r = theta_rd;
        let lam = 0.2998 / f;
        let ce = 1.0 / ae;
        let nu = |i: usize| {
            (h[i] + 500.0 * ce * d[i] * (dtot - d[i]) - (hts * (dtot - d[i]) + hrs * d[i]) / dtot)
                * (0.002 * dtot / (lam * d[i] * (dtot - d[i]))).sqrt()
        };
        let numax = inner.clone().map(nu).fold(f64::NEG_INFINITY, f64::max);
        lt = inner.clone().rfind(|&i| nu(i) == numax).unwrap_or(1);
        dlt = d[lt];
        dlr = dtot - dlt;
        lr = inner.clone().rfind(|&i| dlr <= dtot - d[i]).unwrap_or(lt);
    }
    let theta = 1e3 * dtot / ae + theta_t + theta_r;
    hst = hst.min(h[0]);
    hsr = hsr.min(h[n - 1]);
    let m = (hsr - hst) / dtot;
    let hte = htg + h[0] - hst;
    let hre = hrg + h[n - 1] - hsr;
    let hm = (lt..=lr).map(|i| h[i] - (hst + m * d[i])).fold(f64::NEG_INFINITY, f64::max);
    Smooth { hstd, hsrd, hte, hre, hm, dlt, dlr, theta_t, theta_r, theta, transhorizon }
}

fn knife(nu: f64) -> f64 {
    if nu > -0.78 {
        6.9 + 20.0 * (((nu - 0.1).powi(2) + 1.0).sqrt() + nu - 0.1).log10()
    } else {
        0.0
    }
}

pub fn bullington(d: &[f64], h: &[f64], hts: f64, hrs: f64, ap: f64, f: f64) -> f64 {
    let n = d.len();
    let ce = 1.0 / ap;
    let lam = 0.2998 / f;
    let dtot = d[n - 1] - d[0];
    let inner = 1..n - 1;
    let bulge = |i: usize| h[i] + 500.0 * ce * d[i] * (dtot - d[i]);
    let stim = inner.clone().map(|i| (bulge(i) - hts) / d[i]).fold(f64::NEG_INFINITY, f64::max);
    let str_ = (hrs - hts) / dtot;
    let luc = if stim < str_ {
        let numax = inner
            .clone()
            .map(|i| {
                (bulge(i) - (hts * (dtot - d[i]) + hrs * d[i]) / dtot)
                    * (0.002 * dtot / (lam * d[i] * (dtot - d[i]))).sqrt()
            })
            .fold(f64::NEG_INFINITY, f64::max);
        knife(numax)
    } else {
        let srim = inner
            .clone()
            .map(|i| (bulge(i) - hrs) / (dtot - d[i]))
            .fold(f64::NEG_INFINITY, f64::max);
        let dbp = (hrs - hts + srim * dtot) / (stim + srim);
        let nub = (hts + stim * dbp - (hts * (dtot - dbp) + hrs * dbp) / dtot)
            * (0.002 * dtot / (lam * dbp * (dtot - dbp))).sqrt();
        knife(nub)
    };
    luc + (1.0 - (-luc / 6.0).exp()) * (10.0 + 0.02 * dtot)
}

fn se_first_term_inner(
    epsr: f64,
    sigma: f64,
    d: f64,
    hte: f64,
    hre: f64,
    adft: f64,
    f: f64,
) -> [f64; 2] {
    let k0 = 0.036
        * (adft * f).powf(-1.0 / 3.0)
        * ((epsr - 1.0).powi(2) + (18.0 * sigma / f).powi(2)).powf(-0.25);
    let k = [k0, k0 * (epsr * epsr + (18.0 * sigma / f).powi(2)).sqrt()];
    let mut out = [0.0; 2];
    for i in 0..2 {
        let kk = k[i];
        let beta = (1.0 + 1.6 * kk.powi(2) + 0.67 * kk.powi(4))
            / (1.0 + 4.5 * kk.powi(2) + 1.53 * kk.powi(4));
        let x = 21.88 * beta * (f / (adft * adft)).powf(1.0 / 3.0) * d;
        let yt = 0.9575 * beta * (f * f / adft).powf(1.0 / 3.0) * hte;
        let yr = 0.9575 * beta * (f * f / adft).powf(1.0 / 3.0) * hre;
        let fx = if x >= 1.6 {
            11.0 + 10.0 * x.log10() - 17.6 * x
        } else {
            -20.0 * x.log10() - 5.6488 * x.powf(1.425)
        };
        let g = |y: f64| {
            let b = beta * y;
            let v = if b > 2.0 {
                17.6 * (b - 1.1).sqrt() - 5.0 * (b - 1.1).log10() - 8.0
            } else {
                20.0 * (b + 0.1 * b.powi(3)).log10()
            };
            v.max(2.0 + 20.0 * kk.log10())
        };
        out[i] = -fx - g(yt) - g(yr);
    }
    out
}

fn se_first_term(d: f64, hte: f64, hre: f64, adft: f64, f: f64, omega: f64) -> [f64; 2] {
    let land = se_first_term_inner(22.0, 0.003, d, hte, hre, adft, f);
    let sea = se_first_term_inner(80.0, 5.0, d, hte, hre, adft, f);
    [0, 1].map(|i| omega * sea[i] + (1.0 - omega) * land[i])
}

pub fn spherical(d: f64, hte: f64, hre: f64, ap: f64, f: f64, omega: f64) -> [f64; 2] {
    let lam = 0.2998 / f;
    let dlos = (2.0 * ap).sqrt() * ((0.001 * hte).sqrt() + (0.001 * hre).sqrt());
    if d >= dlos {
        return se_first_term(d, hte, hre, ap, f, omega);
    }
    let c = (hte - hre) / (hte + hre);
    let m = 250.0 * d * d / (ap * (hte + hre));
    let b = 2.0
        * ((m + 1.0) / (3.0 * m)).sqrt()
        * (PI / 3.0 + (1.5 * c * (3.0 * m / (m + 1.0).powi(3)).sqrt()).acos() / 3.0).cos();
    let dse1 = d / 2.0 * (1.0 + b);
    let dse2 = d - dse1;
    let hse =
        ((hte - 500.0 * dse1 * dse1 / ap) * dse2 + (hre - 500.0 * dse2 * dse2 / ap) * dse1) / d;
    let hreq = 17.456 * (dse1 * dse2 * lam / d).sqrt();
    if hse > hreq {
        return [0.0; 2];
    }
    let aem = 500.0 * (d / (hte.sqrt() + hre.sqrt())).powi(2);
    let ldft = se_first_term(d, hte, hre, aem, f, omega);
    ldft.map(|v| (1.0 - hse / hreq) * v.max(0.0))
}

fn delta_bullington(
    d: &[f64],
    h: &[f64],
    hts: f64,
    hrs: f64,
    hstd: f64,
    hsrd: f64,
    ap: f64,
    f: f64,
    omega: f64,
) -> [f64; 2] {
    let lbulla = bullington(d, h, hts, hrs, ap, f);
    let (hts1, hrs1) = (hts - hstd, hrs - hsrd);
    let flat = vec![0.0; h.len()];
    let lbulls = bullington(d, &flat, hts1, hrs1, ap, f);
    let ldsph = spherical(d[d.len() - 1] - d[0], hts1, hrs1, ap, f, omega);
    ldsph.map(|v| lbulla + (v - lbulls).max(0.0))
}

fn diffraction(
    d: &[f64],
    h: &[f64],
    hts: f64,
    hrs: f64,
    hstd: f64,
    hsrd: f64,
    f: f64,
    omega: f64,
    p: f64,
    b0: f64,
    dn: f64,
) -> ([f64; 2], [f64; 2]) {
    let (ae, ab) = earth_radius(dn);
    let ld50 = delta_bullington(d, h, hts, hrs, hstd, hsrd, ae, f, omega);
    if p == 50.0 {
        return (ld50, ld50);
    }
    let ldb = delta_bullington(d, h, hts, hrs, hstd, hsrd, ab, f, omega);
    let fi = if p > b0 { inv_cum_norm(p / 100.0) / inv_cum_norm(b0 / 100.0) } else { 1.0 };
    ([0, 1].map(|i| ld50[i] + fi * (ldb[i] - ld50[i])), ld50)
}

fn line_of_sight(
    d: f64,
    f: f64,
    p: f64,
    b0: f64,
    w: f64,
    temp: f64,
    press: f64,
    dlt: f64,
    dlr: f64,
) -> (f64, f64, f64) {
    let (g_o, g_w) = p676_gaseous(f, press, 7.5 + 2.5 * w, temp + 273.15);
    let lbfsg = 92.4 + 20.0 * f.log10() + 20.0 * d.log10() + (g_o + g_w) * d;
    let k = 2.6 * (1.0 - (-0.1 * (dlt + dlr)).exp());
    (lbfsg, lbfsg + k * (p / 50.0).log10(), lbfsg + k * (b0 / 50.0).log10())
}

fn troposcatter(
    dtot: f64,
    theta: f64,
    f: f64,
    p: f64,
    temp: f64,
    press: f64,
    n0: f64,
    gt: f64,
    gr: f64,
) -> f64 {
    let lf = 25.0 * f.log10() - 2.5 * (f / 2.0).log10().powi(2);
    let lc = 0.051 * (0.055 * (gt + gr)).exp();
    let (g_o, g_w) = p676_gaseous(f, press, 3.0, temp + 273.15);
    let ag = (g_o + g_w) * dtot;
    190.0 + lf + 20.0 * dtot.log10() + 0.573 * theta - 0.15 * n0 + lc + ag
        - 10.1 * (-(p / 50.0).log10()).powf(0.7)
}

fn anomalous(
    dtot: f64,
    s: &Smooth,
    dct: f64,
    dcr: f64,
    dlm: f64,
    hts: f64,
    hrs: f64,
    f: f64,
    p: f64,
    temp: f64,
    press: f64,
    omega: f64,
    ae: f64,
    b0: f64,
) -> f64 {
    let alf = if f < 0.5 { 45.375 - 137.0 * f + 92.5 * f * f } else { 0.0 };
    let shield = |theta: f64, dl: f64| {
        let t1 = theta - 0.1 * dl;
        if t1 > 0.0 {
            20.0 * (1.0 + 0.361 * t1 * (f * dl).sqrt()).log10() + 0.264 * t1 * f.powf(1.0 / 3.0)
        } else {
            0.0
        }
    };
    let ast = shield(s.theta_t, s.dlt);
    let asr = shield(s.theta_r, s.dlr);
    let coupling = |dc: f64, dl: f64, hs: f64| {
        if dc <= 5.0 && dc <= dl && omega >= 0.75 {
            -3.0 * (-0.25 * dc * dc).exp() * (1.0 + (0.07 * (50.0 - hs)).tanh())
        } else {
            0.0
        }
    };
    let act = coupling(dct, s.dlt, hts);
    let acr = coupling(dcr, s.dlr, hrs);
    let gamma_d = 5e-5 * ae * f.powf(1.0 / 3.0);
    let theta_t1 = if s.theta_t > 0.1 * s.dlt { 0.1 * s.dlt } else { s.theta_t };
    let theta_r1 = if s.theta_r > 0.1 * s.dlr { 0.1 * s.dlr } else { s.theta_r };
    let theta1 = 1e3 * dtot / ae + theta_t1 + theta_r1;
    let di = (dtot - s.dlt - s.dlr).min(40.0);
    let mu3 = if s.hm > 10.0 { (-4.6e-5 * (s.hm - 10.0) * (43.0 + 6.0 * di)).exp() } else { 1.0 };
    let tau = 1.0 - (-(4.12e-4 * dlm.powf(2.41))).exp();
    let alpha = (-0.6 - 3.5e-9 * dtot.powf(3.1) * tau).max(-3.4);
    let mu2 =
        (500.0 / ae * dtot * dtot / (s.hte.sqrt() + s.hre.sqrt()).powi(2)).powf(alpha).min(1.0);
    let beta = b0 * mu2 * mu3;
    let lb = beta.log10();
    let gamma = 1.076 / (2.0058 - lb).powf(1.012)
        * (-(9.51 - 4.8 * lb + 0.198 * lb * lb) * 1e-6 * dtot.powf(1.13)).exp();
    let ap = -12.0 + (1.2 + 3.7e-3 * dtot) * (p / beta).log10() + 12.0 * (p / beta).powf(gamma);
    let adp = gamma_d * theta1 + ap;
    let (g_o, g_w) = p676_gaseous(f, press, 7.5 + 2.5 * omega, temp + 273.15);
    let ag = (g_o + g_w) * dtot;
    let af =
        102.45 + 20.0 * f.log10() + 20.0 * (s.dlt + s.dlr).log10() + alf + ast + asr + act + acr;
    af + adp + ag
}

pub fn bt_loss(inp: &Input) -> Result<Output, String> {
    let (d, h) = (inp.d, inp.h);
    let n = d.len();
    if n < 4 || d[0] != 0.0 || d.windows(2).any(|w| w[1] < w[0]) {
        return Err("P.452 needs at least 4 profile points starting at 0 km".into());
    }
    if !(inp.p > 0.0 && inp.p <= 50.0) {
        return Err("P.452 covers 0 to 50% of the time".into());
    }
    let dtot = d[n - 1];
    let mut g = inp.g.to_vec();
    for i in 0..n {
        if d[i] < 0.05 || d[i] > dtot - 0.05 {
            g[i] = h[i];
        }
    }
    let land: Vec<bool> = inp.zone.iter().map(|z| *z != Zone::Sea).collect();
    let inland: Vec<bool> = inp.zone.iter().map(|z| *z == Zone::Inland).collect();
    let dtm = longest_section(d, &land);
    let dlm = longest_section(d, &inland);
    let b0 = beta0(inp.mid_lat, dtm, dlm);
    let (ae, _) = earth_radius(inp.dn);
    let omega = path_fraction(d, inp.zone, Zone::Sea);
    let f = inp.f_ghz;
    let s = smooth_earth_heights(d, h, inp.htg, inp.hrg, ae, f);
    let hts = h[0] + inp.htg;
    let hrs = h[n - 1] + inp.hrg;
    let ce = 1.0 / ae;
    let stim = (1..n - 1)
        .map(|i| (h[i] + 500.0 * ce * d[i] * (dtot - d[i]) - hts) / d[i])
        .fold(f64::NEG_INFINITY, f64::max);
    let str_ = (hrs - hts) / dtot;
    let fj = 1.0 - 0.5 * (1.0 + (3.0 * 0.8 * (stim - str_) / 0.3).tanh());
    let fk = 1.0 - 0.5 * (1.0 + (3.0 * 0.5 * (dtot - 20.0) / 20.0).tanh());
    let d3d = (dtot * dtot + ((hts - hrs) / 1000.0).powi(2)).sqrt();
    let (lbfsg, lb0p, lb0b) =
        line_of_sight(d3d, f, inp.p, b0, omega, inp.temp, inp.press, s.dlt, s.dlr);
    let (ldp, ld50) = diffraction(d, &g, hts, hrs, s.hstd, s.hsrd, f, omega, inp.p, b0, inp.dn);
    let lbd50 = ld50.map(|v| lbfsg + v);
    let lbd = ldp.map(|v| lb0p + v);
    let mut lminb0p = ldp.map(|v| lb0p + (1.0 - omega) * v);
    if inp.p >= b0 {
        let fi = inv_cum_norm(inp.p / 100.0) / inv_cum_norm(b0 / 100.0);
        lminb0p = [0, 1].map(|i| lbd50[i] + (lb0b + (1.0 - omega) * ldp[i] - lbd50[i]) * fi);
    }
    let eta = 2.5;
    let lba = anomalous(
        dtot, &s, inp.dct, inp.dcr, dlm, hts, hrs, f, inp.p, inp.temp, inp.press, omega, ae, b0,
    );
    let lminbap = eta * ((lba / eta).exp() + (lb0p / eta).exp()).ln();
    let lbda = lbd.map(|v| if lminbap <= v { lminbap + (v - lminbap) * fk } else { v });
    let lbam = [0, 1].map(|i| lbda[i] + (lminb0p[i] - lbda[i]) * fj);
    let lbs = troposcatter(dtot, s.theta, f, inp.p, inp.temp, inp.press, inp.n0, inp.gt, inp.gr);
    let pol = inp.vertical as usize;
    let lb = -5.0 * (10f64.powf(-0.2 * lbs) + 10f64.powf(-0.2 * lbam[pol])).log10();
    Ok(Output { lb, lbfsg, lb0p, ldp: ldp[pol], lbs, lba, transhorizon: s.transhorizon })
}

pub fn lat_along(lat_t: f64, lon_t: f64, lat_r: f64, lon_r: f64, dist_km: f64) -> f64 {
    let (pt, pr) = (lat_t.to_radians(), lat_r.to_radians());
    let dl = (lon_r - lon_t).to_radians();
    let r = pt.sin() * pr.sin() + pt.cos() * pr.cos() * dl.cos();
    let x1 = pr.sin() - r * pt.sin();
    let y1 = pt.cos() * pr.cos() * dl.sin();
    let bearing =
        if x1.abs() < 1e-9 && y1.abs() < 1e-9 { lon_r.to_radians() } else { y1.atan2(x1) };
    let phi = dist_km / 6371.0;
    let s = pt.sin() * phi.cos() + pt.cos() * phi.sin() * bearing.cos();
    s.asin().to_degrees()
}

pub struct Climate {
    pub dn: f64,
    pub n0: f64,
    pub press: f64,
    pub temp: f64,
}

pub fn zones(ground: &[f64], dist_km: &[f64]) -> Vec<Zone> {
    let sea: Vec<bool> = ground.iter().map(|g| *g <= 0.5).collect();
    let sea_at: Vec<f64> = dist_km.iter().zip(&sea).filter(|(_, s)| **s).map(|(d, _)| *d).collect();
    ground
        .iter()
        .zip(dist_km)
        .zip(&sea)
        .map(|((g, d), s)| {
            if *s {
                Zone::Sea
            } else if *g < 100.0 && sea_at.iter().any(|x| (x - d).abs() <= 50.0) {
                Zone::Coastal
            } else {
                Zone::Inland
            }
        })
        .collect()
}

pub fn for_profile(
    p: &crate::Profile,
    f_mhz: f64,
    time_pct: f64,
    climate: &Climate,
    vertical: bool,
    gt: f64,
    gr: f64,
) -> Result<Output, String> {
    let d: Vec<f64> = p.samples.iter().map(|s| s.dist / 1000.0).collect();
    let h: Vec<f64> = p.samples.iter().map(|s| s.ground).collect();
    let zone = zones(&h, &d);
    let dtot = d[d.len() - 1];
    let first_sea = zone.iter().position(|z| *z == Zone::Sea);
    let last_sea = zone.iter().rposition(|z| *z == Zone::Sea);
    let dct = first_sea.map_or(500.0, |i| d[i]);
    let dcr = last_sea.map_or(500.0, |i| dtot - d[i]);
    let mid_lat = lat_along(p.a.at.lat, p.a.at.lon, p.b.at.lat, p.b.at.lon, dtot / 2.0);
    bt_loss(&Input {
        f_ghz: f_mhz / 1000.0,
        p: time_pct,
        d: &d,
        h: &h,
        g: &h,
        zone: &zone,
        htg: p.a.height_agl,
        hrg: p.b.height_agl,
        mid_lat,
        gt,
        gr,
        vertical,
        dct,
        dcr,
        press: climate.press,
        temp: climate.temp,
        dn: climate.dn,
        n0: climate.n0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(path: &str) -> Vec<Vec<String>> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').map(|v| v.trim().to_string()).collect())
            .collect()
    }

    #[test]
    fn matches_the_itu_validation_examples() {
        let dir = format!("{}/tests/data/p452", env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        let mut worst: f64 = 0.0;
        for entry in std::fs::read_dir(format!("{dir}/profiles")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let prof = csv(path.to_str().unwrap());
            let d: Vec<f64> = prof.iter().map(|r| r[0].parse().unwrap()).collect();
            let h: Vec<f64> = prof.iter().map(|r| r[1].parse().unwrap()).collect();
            let gc: Vec<f64> = prof.iter().map(|r| r[2].parse().unwrap()).collect();
            let zone: Vec<Zone> = prof
                .iter()
                .map(|r| match r[4].as_str() {
                    "1" => Zone::Coastal,
                    "2" => Zone::Inland,
                    _ => Zone::Sea,
                })
                .collect();
            let g: Vec<f64> = h.iter().zip(&gc).map(|(a, b)| a + b).collect();
            let res =
                csv(&format!("{dir}/results/{}", name.replace("test_profile", "test_result")));
            for r in res {
                let v = |i: usize| r[i].parse::<f64>().unwrap();
                let inp = Input {
                    f_ghz: v(1),
                    p: v(2),
                    d: &d,
                    h: &h,
                    g: &g,
                    zone: &zone,
                    htg: v(3),
                    hrg: v(4),
                    mid_lat: lat_along(v(6), v(5), v(8), v(7), d[d.len() - 1] / 2.0),
                    gt: v(9),
                    gr: v(10),
                    vertical: v(11) == 2.0,
                    dct: v(12),
                    dcr: v(13),
                    press: v(14),
                    temp: v(15),
                    dn: v(35),
                    n0: v(36),
                };
                let out = bt_loss(&inp).unwrap();
                let err = (out.lb - v(37)).abs();
                worst = worst.max(err);
                assert!(err < 1e-4, "{name} f {} p {}: {} vs {}", v(1), v(2), out.lb, v(37));
                checked += 1;
            }
        }
        eprintln!("{checked} cases, worst {worst:.2e} dB");
        assert!(checked > 500);
    }
}
