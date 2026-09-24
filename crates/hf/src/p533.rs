use crate::p372::{self, Coefficients, ManMade, Noise};
use crate::tables::*;
use crate::{D2R, PI, R0, R2D, TINYDB};

const TOOBIG: f64 = f64::MAX;
const HR100: usize = 0;
const HR300: usize = 1;
const T1K: usize = 0;
const TD02: usize = 1;
const MP: usize = 2;
const RD02: usize = 3;
const R1K: usize = 4;
const WINTER: usize = 0;
const EQUINOX: usize = 1;
const SUMMER: usize = 2;
const NOLOWEST: usize = 99;
const MAXF2: usize = 6;
const MAXE: usize = 3;
const MINELE: f64 = 3.0;
const MAXSSN: i32 = 160;

#[derive(Clone, Copy, Debug, Default)]
pub struct Location {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Sun {
    ha: f64,
    sha: f64,
    sza: f64,
    decl: f64,
    eot: f64,
    lsr: f64,
    lsn: f64,
    lss: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct ControlPt {
    l: Location,
    distance: f64,
    fo_e: f64,
    fo_f2: f64,
    m3k: f64,
    dip: [f64; 2],
    fh: [f64; 2],
    ltime: f64,
    hr: f64,
    x: f64,
    sun: Sun,
}

#[derive(Clone, Copy, Debug)]
pub struct Mode {
    pub bmuf: f64,
    pub muf90: f64,
    pub muf50: f64,
    pub muf10: f64,
    pub opmuf: f64,
    pub opmuf10: f64,
    pub opmuf90: f64,
    pub fprob: f64,
    pub deltal: f64,
    pub deltau: f64,
    pub hr: f64,
    pub fs: f64,
    pub lb: f64,
    pub ew: f64,
    pub ele: f64,
    pub prw: f64,
    pub grw: f64,
}

impl Default for Mode {
    fn default() -> Self {
        Mode {
            bmuf: 0.0,
            muf90: 0.0,
            muf50: 0.0,
            muf10: 0.0,
            opmuf: 0.0,
            opmuf10: 0.0,
            opmuf90: 0.0,
            fprob: 0.0,
            deltal: 0.0,
            deltau: 0.0,
            hr: 0.0,
            fs: 0.0,
            lb: -TINYDB,
            ew: TINYDB,
            ele: 0.0,
            prw: TINYDB,
            grw: TINYDB,
        }
    }
}

#[derive(Clone)]
pub struct Pattern {
    db: Vec<f64>,
}

impl Pattern {
    pub fn isotropic(g: f64) -> Pattern {
        Pattern { db: vec![g; 360 * 91] }
    }

    pub fn from_fn(f: impl Fn(f64, f64) -> f64) -> Pattern {
        let mut db = vec![0.0; 360 * 91];
        for az in 0..360 {
            for el in 0..91 {
                db[az * 91 + el] = f(az as f64, el as f64);
            }
        }
        Pattern { db }
    }

    fn at(&self, az: usize, el: usize) -> f64 {
        self.db[(az % 360) * 91 + el.min(90)]
    }
}

pub struct IonMaps {
    fof2: Vec<f32>,
    m3k: Vec<f32>,
}

const HOURS: usize = 24;
const LNG: usize = 241;
const LAT: usize = 121;

impl IonMaps {
    pub fn from_bin(bytes: &[u8]) -> Result<IonMaps, String> {
        let n = HOURS * LNG * LAT * 2;
        if bytes.len() < 15 + 8 * n {
            return Err(format!("ionospheric map is {} bytes, too short", bytes.len()));
        }
        let read = |start: usize| -> Vec<f32> {
            let raw: Vec<f32> = bytes[start..start + 4 * n]
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let mut out = vec![0f32; n];
            for m in 0..2 {
                for j in 0..LNG {
                    for k in 0..LAT {
                        for i in 0..HOURS {
                            out[Self::idx(i, j, k, m)] =
                                raw[m * (LNG * LAT * HOURS) + j * (LAT * HOURS) + k * HOURS + i];
                        }
                    }
                }
            }
            out
        };
        Ok(IonMaps { fof2: read(5), m3k: read(5 + 4 * n + 10) })
    }

    fn idx(i: usize, j: usize, k: usize, m: usize) -> usize {
        ((i * LNG + j) * LAT + k) * 2 + m
    }
}

pub struct Deciles {
    v: Vec<f64>,
}

impl Default for Deciles {
    fn default() -> Self {
        let text = include_str!("../data/decile.txt");
        let rows: Vec<Vec<f64>> = text
            .lines()
            .map(|l| l.split_whitespace().filter_map(|t| t.parse().ok()).collect())
            .collect();
        let mut v = vec![0.0; 3 * 24 * 19 * 3 * 2];
        let mut r = 0;
        for n in 0..2 {
            for i in 0..3 {
                for m in 0..3 {
                    for k in (0..19).rev() {
                        for h in 0..24 {
                            v[Self::idx(i, h, k, m, n)] = rows[r][h];
                        }
                        r += 1;
                    }
                }
            }
        }
        Deciles { v }
    }
}

impl Deciles {
    fn idx(season: usize, hour: usize, lat: usize, ssn: usize, decile: usize) -> usize {
        (((season * 24 + hour) * 19 + lat) * 3 + ssn) * 2 + decile
    }

    fn get(&self, season: usize, hour: usize, lat: usize, ssn: usize, decile: usize) -> f64 {
        self.v[Self::idx(season, hour, lat, ssn, decile)]
    }
}

#[derive(Clone)]
pub struct Input {
    pub month: usize,
    pub hour: usize,
    pub ssn: i32,
    pub frequency: f64,
    pub bw: f64,
    pub txpower: f64,
    pub snrr: f64,
    pub snrxxp: i32,
    pub tx: Location,
    pub rx: Location,
    pub long_path: bool,
    pub man_made: ManMade,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub distance: f64,
    pub dmax: f64,
    pub ptick: f64,
    pub ele: f64,
    pub bmuf: f64,
    pub muf50: f64,
    pub muf90: f64,
    pub muf10: f64,
    pub opmuf: f64,
    pub opmuf90: f64,
    pub opmuf10: f64,
    pub n0_f2: Option<usize>,
    pub n0_e: Option<usize>,
    pub ep: f64,
    pub es: f64,
    pub el: f64,
    pub pr: f64,
    pub grw: f64,
    pub noise: Noise,
    pub snr: f64,
    pub du_sn: f64,
    pub dl_sn: f64,
    pub snrxx: f64,
    pub bcr: f64,
    pub f2: [Mode; MAXF2],
    pub e: [Mode; MAXE],
    pub dominant: Option<usize>,
}

pub struct Context<'a> {
    pub maps: &'a IonMaps,
    pub deciles: &'a Deciles,
    pub noise: &'a Coefficients,
    pub tx_ant: &'a Pattern,
    pub rx_ant: &'a Pattern,
}

struct Path<'a> {
    ctx: &'a Context<'a>,
    month: usize,
    hour: usize,
    ssn: i32,
    frequency: f64,
    txpower: f64,
    l_tx: Location,
    l_rx: Location,
    long: bool,
    season: usize,
    distance: f64,
    ptick: f64,
    dmax: f64,
    ele: f64,
    bmuf: f64,
    muf50: f64,
    muf90: f64,
    muf10: f64,
    opmuf: f64,
    opmuf90: f64,
    opmuf10: f64,
    n0_f2: usize,
    n0_e: usize,
    es: f64,
    el: f64,
    ei: f64,
    ep: f64,
    pr: f64,
    e0: f64,
    gap: f64,
    ly: f64,
    fm: f64,
    fl: f64,
    fh: f64,
    gtl: f64,
    k: [f64; 2],
    grw: f64,
    cp: [ControlPt; 5],
    md_f2: [Mode; MAXF2],
    md_e: [Mode; MAXE],
    dm: Option<usize>,
}

fn great_circle_distance(a: Location, b: Location) -> f64 {
    2.0 * R0
        * (((a.lat - b.lat) / 2.0).sin().powi(2)
            + a.lat.cos() * b.lat.cos() * ((a.lng - b.lng) / 2.0).sin().powi(2))
        .sqrt()
        .asin()
}

fn great_circle_point(a: Location, b: Location, cp: &mut ControlPt, distance: f64, fraction: f64) {
    if distance != 0.0 {
        cp.distance = distance * fraction;
        let d = distance / R0;
        let aa = ((1.0 - fraction) * d).sin() / d.sin();
        let bb = (fraction * d).sin() / d.sin();
        let x = aa * a.lat.cos() * a.lng.cos() + bb * b.lat.cos() * b.lng.cos();
        let y = aa * a.lat.cos() * a.lng.sin() + bb * b.lat.cos() * b.lng.sin();
        let z = aa * a.lat.sin() + bb * b.lat.sin();
        cp.l.lat = z.atan2((x * x + y * y).sqrt());
        cp.l.lng = y.atan2(x);
    } else {
        cp.distance = 0.0;
        cp.l = a;
    }
}

fn geomagnetic(here: Location) -> Location {
    let (plat, plng) = (78.5 * D2R, -68.2 * D2R);
    let lat = (here.lat.sin() * plat.sin() + here.lat.cos() * plat.cos() * (here.lng - plng).cos())
        .asin();
    let lng = (here.lat.cos() * (here.lng - plng).sin() / lat.cos()).asin();
    Location { lat, lng }
}

pub fn bearing(here: Location, there: Location, long: bool) -> f64 {
    let num = (there.lng - here.lng).sin() * there.lat.cos();
    let den = here.lat.cos() * there.lat.sin()
        - here.lat.sin() * there.lat.cos() * (there.lng - here.lng).cos();
    let mut b = (2.0 * PI + num.atan2(den)) % (2.0 * PI);
    if long {
        b = (2.0 * PI + (b + PI)) % (2.0 * PI);
    }
    b
}

fn what_season(l: Location, month: usize) -> usize {
    let north = l.lat >= 0.0;
    match month {
        10 | 11 | 0 | 1 => {
            if north {
                WINTER
            } else {
                SUMMER
            }
        }
        2 | 3 | 8 | 9 => EQUINOX,
        _ => {
            if north {
                SUMMER
            } else {
                WINTER
            }
        }
    }
}

fn bilinear(ll: f64, lr: f64, ul: f64, ur: f64, r: f64, c: f64) -> f64 {
    ll * ((1.0 - r) * (1.0 - c)) + ul * (r * (1.0 - c)) + lr * ((1.0 - r) * c) + ur * (r * c)
}

#[derive(Clone, Copy, Default)]
struct Cell {
    j: i32,
    k: i32,
}

fn ionospheric(cp: &mut ControlPt, maps: &IonMaps, hour: usize, ssn: i32) {
    let inc = 1.5 * D2R;
    let (lng, lat) = (241, 121);
    let (zlat, zlng) = (60, 120);
    let (mut ul, mut ur, mut lr, mut ll): (Cell, Cell, Cell, Cell);
    let kk = zlat + (cp.l.lat / inc) as i32;
    let jj = zlng + (cp.l.lng / inc) as i32;
    if cp.l.lat >= 0.0 {
        if cp.l.lng >= 0.0 {
            ll = Cell { k: kk, j: jj };
            lr = Cell { k: ll.k, j: ll.j + 1 };
            ur = Cell { k: ll.k + 1, j: ll.j + 1 };
            ul = Cell { k: ll.k + 1, j: ll.j };
            if ll.j != lng - 1 {
                if ll.k == lat - 1 {
                    ur.k = ll.k;
                    ul.k = ll.k;
                }
            } else if ll.k != lat - 1 {
                lr.j = 0;
                ur.j = 0;
            } else {
                lr = Cell { k: ll.k, j: 0 };
                ur = Cell { k: ll.k, j: 0 };
                ul = Cell { k: ll.k, j: ll.j };
            }
        } else {
            lr = Cell { k: kk, j: jj };
            ll = Cell { k: lr.k, j: lr.j - 1 };
            ul = Cell { k: lr.k + 1, j: lr.j - 1 };
            ur = Cell { k: lr.k + 1, j: lr.j };
            if lr.j != 0 {
                if lr.k == lat - 1 {
                    ur.k = lr.k;
                    ul.k = lr.k;
                }
            } else if lr.k != lat - 1 {
                ll.j = lng - 1;
                ul.j = lng - 1;
            } else {
                ll = Cell { k: lr.k, j: lng - 1 };
                ur = Cell { k: lr.k, j: lr.j };
                ul = Cell { k: lr.k, j: lng - 1 };
            }
        }
    } else if cp.l.lng >= 0.0 {
        ul = Cell { k: kk, j: jj };
        ur = Cell { k: ul.k, j: ul.j + 1 };
        ll = Cell { k: ul.k - 1, j: ul.j };
        lr = Cell { k: ul.k - 1, j: ul.j + 1 };
        if ul.j != lng - 1 {
            if ul.k == 0 {
                ll.k = ul.k;
                lr.k = ul.k;
            }
        } else if ul.k != 0 {
            lr.j = 0;
            ur.j = 0;
        } else {
            lr = Cell { k: ul.k, j: 0 };
            ur = Cell { k: ul.k, j: 0 };
            ll = Cell { k: ul.k, j: ul.j };
        }
    } else {
        ur = Cell { k: kk, j: jj };
        ul = Cell { k: ur.k, j: ur.j - 1 };
        ll = Cell { k: ur.k - 1, j: ur.j - 1 };
        lr = Cell { k: ur.k - 1, j: ur.j };
        if ur.j != 0 {
            if ur.k == 0 {
                lr.k = ur.k;
                ll.k = ur.k;
            }
        } else if ur.k != 0 {
            ll.j = lng - 1;
            ul.j = lng - 1;
        } else {
            lr = Cell { k: ur.k, j: ur.j };
            ll = Cell { k: ur.k, j: lng - 1 };
            ul = Cell { k: ur.k, j: lng - 1 };
        }
    }
    let get = |v: &Vec<f32>, c: Cell, m: usize| {
        v[IonMaps::idx(hour, c.j as usize, c.k as usize, m)] as f64
    };
    let frack = (cp.l.lat / inc).abs() - ((cp.l.lat / inc).abs() as i32) as f64;
    let fracj = (cp.l.lng / inc).abs() - ((cp.l.lng / inc).abs() as i32) as f64;
    let mut f = [0.0; 2];
    let mut m3 = [0.0; 2];
    for m in 0..2 {
        f[m] = bilinear(
            get(&maps.fof2, ll, m),
            get(&maps.fof2, lr, m),
            get(&maps.fof2, ul, m),
            get(&maps.fof2, ur, m),
            frack,
            fracj,
        );
        m3[m] = bilinear(
            get(&maps.m3k, ll, m),
            get(&maps.m3k, lr, m),
            get(&maps.m3k, ul, m),
            get(&maps.m3k, ur, m),
            frack,
            fracj,
        );
    }
    let ssn = ssn.min(MAXSSN) as f64;
    cp.fo_f2 = (f[1] * ssn + f[0] * (100.0 - ssn)) / 100.0;
    cp.m3k = (m3[1] * ssn + m3[0] * (100.0 - ssn)) / 100.0;
}

fn solar(cp: &mut ControlPt, month: usize, hour: f64) {
    let a = 0.98565327;
    let b = 3.98891967;
    let s = (23.45 * D2R).sin();
    let c = (23.45 * D2R).cos();
    let v = 78.746118 * D2R;
    let doty = [0, 31, 59, 90, 120, 152, 181, 212, 243, 273, 304, 334];
    let tz = ((cp.l.lng / (15.0 * D2R)) as i32) as f64;
    let ltime = hour + tz;
    let d = doty[month] as f64 + 15.0 + hour / 24.0;
    let lambda = a * D2R * (d - 2.0);
    let nu = lambda + 1.915169 * D2R * lambda.sin();
    let mut eps = a * D2R * (d - 80.0);
    if eps >= 270.0 * D2R {
        eps -= 2.0 * PI;
    } else if eps >= 90.0 * D2R {
        eps -= PI;
    }
    let beta = (c * eps.tan()).atan();
    cp.sun.eot = b * ((eps - beta) + (lambda - nu)) * R2D;
    cp.sun.decl =
        (s * (((a * (d - 2.0) * D2R).sin() * 0.016713 + a * (d - 2.0) * D2R) - v).sin()).asin();
    let toffset = ((cp.l.lng / (15.0 * D2R)) - tz) * 60.0 + cp.sun.eot;
    let tst = ltime * 60.0 + toffset;
    cp.sun.ha = ((tst / 4.0) - 180.0) * D2R;
    cp.sun.sha = (((90.833 * D2R).cos() / (cp.l.lat.cos() * cp.sun.decl.cos()))
        - (cp.l.lat.tan() * cp.sun.decl.tan()))
    .acos();
    let mut cosphi =
        cp.l.lat.sin() * cp.sun.decl.sin() + cp.l.lat.cos() * cp.sun.decl.cos() * cp.sun.ha.cos();
    if cosphi.abs() > 1.0 {
        cosphi = if cosphi >= 0.0 { 1.0 } else { -1.0 };
    }
    cp.sun.sza = cosphi.acos();
    let fm = |x: f64| (x + 24.0) % 24.0;
    cp.sun.lsr = fm((720.0 + (-cp.l.lng - cp.sun.sha) * R2D * 4.0 - cp.sun.eot) / 60.0);
    cp.sun.lss = fm((720.0 + (-cp.l.lng + cp.sun.sha) * R2D * 4.0 - cp.sun.eot) / 60.0);
    cp.sun.lsn = fm((720.0 + (-cp.l.lng) * R2D * 4.0 - cp.sun.eot) / 60.0);
    cp.ltime = hour;
}

fn find_fo_e(cp: &mut ControlPt, month: usize, hour: i32, ssn: i32) {
    let ssn = ssn.min(MAXSSN) as f64;
    let phi = 63.7 + 0.728 * ssn + 0.00089 * ssn * ssn;
    let a = 1.0 + 0.0094 * (phi - 66.0);
    let lat = cp.l.lat;
    let m = if lat.abs() < 32.0 * D2R { -1.93 + 1.92 * lat.cos() } else { 0.11 - 0.49 * lat.cos() };
    let n = if (lat - cp.sun.decl).abs() < 80.0 * D2R { lat - cp.sun.decl } else { 80.0 * D2R };
    let b = n.cos().powf(m);
    let (x, y) = if lat.abs() < 32.0 * D2R { (23.0, 116.0) } else { (92.0, 35.0) };
    let c = x + y * lat.cos();
    let p = if lat.abs() <= 12.0 * D2R { 1.31 } else { 1.2 };
    let sza = cp.sun.sza;
    let d = if sza <= 73.0 * D2R {
        sza.cos().powf(p)
    } else if sza > 73.0 * D2R && sza < PI / 2.0 {
        let dsza = 6.27e-13 * (sza * R2D - 50.0).powf(8.0) * D2R;
        (sza - dsza).cos().powf(p)
    } else {
        let hour = ((hour + 1) % 24) as f64;
        let (lss, lsr) = (cp.sun.lss, cp.sun.lsr);
        let h = if (lss >= lsr && hour >= lss && hour >= lsr)
            || (lss < lsr && hour >= lss && hour < lsr)
        {
            hour - lss
        } else if lss >= lsr && hour < lss && hour < lsr {
            24.0 - lss + hour
        } else {
            0.0
        };
        if (lat > 72.5622 * D2R && matches!(month, 10 | 11 | 0))
            || (lat < 72.5622 * D2R && matches!(month, 4..=6))
        {
            0.072f64.powf(p) * (25.2 - 0.28 * sza * R2D).exp()
        } else {
            (0.072f64.powf(p) * (-1.4 * h).exp())
                .max(0.072f64.powf(p) * (25.2 - 0.28 * sza * R2D).exp())
        }
    };
    cp.fo_e = (a * b * c * d).powf(0.25).max((0.004 * (1.0 + 0.021 * phi).powi(2)).powf(0.25));
}

fn magfit(cp: &mut ControlPt, height: f64) {
    let mut p = [[0.0f64; 7]; 7];
    let mut dp = [[0.0f64; 7]; 7];
    p[0][0] = 1.0;
    let g = |m: usize, n: usize| MAG_G[m * 7 + n];
    let h = |m: usize, n: usize| MAG_H[m * 7 + n];
    let ct = |m: usize, n: usize| MAG_CT[m * 7 + n];
    let ar = R0 / (R0 + height);
    let (lat, lng) = (cp.l.lat, cp.l.lng);
    let (mut fz, mut fx, mut fy) = (0.0, 0.0, 0.0);
    for n in 1..=6usize {
        let (mut sz, mut sx, mut sy) = (0.0, 0.0, 0.0);
        for m in 0..=n {
            if n == m {
                p[m][n] = lat.cos() * p[m - 1][n - 1];
                dp[m][n] = lat.cos() * dp[m - 1][n - 1] + lat.sin() * p[m - 1][n - 1];
            } else if n != 1 {
                p[m][n] = lat.sin() * p[m][n - 1] - ct(m, n) * p[m][n - 2];
                dp[m][n] =
                    lat.sin() * dp[m][n - 1] - lat.cos() * p[m][n - 1] - ct(m, n) * dp[m][n - 2];
            } else {
                p[m][n] = lat.sin() * p[m][n - 1];
                dp[m][n] = lat.sin() * dp[m][n - 1] - lat.cos() * p[m][n - 1];
            }
            let mf = m as f64;
            sz += p[m][n] * (g(m, n) * (mf * lng).cos() + h(m, n) * (mf * lng).sin());
            sx += dp[m][n] * (g(m, n) * (mf * lng).cos() + h(m, n) * (mf * lng).sin());
            sy += mf * p[m][n] * (g(m, n) * (mf * lng).sin() - h(m, n) * (mf * lng).cos());
        }
        let arn = ar.powi(n as i32 + 2);
        fz += arn * (n as f64 + 1.0) * sz;
        fx -= arn * sx;
        fy += arn * sy;
    }
    let hr = if height == 100.0 { HR100 } else { HR300 };
    cp.dip[hr] = (fz / (fx * fx + (fy / lat.cos()).powi(2)).sqrt()).atan();
    cp.fh[hr] = 2.8 * (fx * fx + (fy / lat.cos()).powi(2) + fz * fz).sqrt();
}

fn elevation_angle(dh: f64, hr: f64) -> f64 {
    ((1.0 / (dh / (2.0 * R0)).tan()) - ((R0 / (R0 + hr)) / (dh / (2.0 * R0)).sin())).atan()
}

fn incidence_angle(deltaf: f64, hr: f64) -> f64 {
    (R0 * deltaf.cos() / (R0 + hr)).asin()
}

fn calc_b(cp: &mut ControlPt) -> f64 {
    cp.x = if cp.fo_e != 0.0 { (cp.fo_f2 / cp.fo_e).max(2.0) } else { 2.0 };
    cp.m3k - 0.124 + (cp.m3k.powi(2) - 4.0) * (0.0215 + 0.005 * ((7.854 / cp.x) - 1.9635).sin())
}

fn calc_dmax(cp: &mut ControlPt) -> f64 {
    let b = calc_b(cp);
    let x = cp.x;
    4780.0
        + (12610.0 + (2140.0 / x.powi(2)) - (49720.0 / x.powi(4)) + (688900.0 / x.powi(6)))
            * ((1.0 / b) - 0.303)
}

fn calc_cd(d: f64, dmax: f64) -> f64 {
    let z = 1.0 - 2.0 * d / dmax;
    0.74 - 0.591 * z - 0.424 * z.powi(2) - 0.090 * z.powi(3)
        + 0.088 * z.powi(4)
        + 0.181 * z.powi(5)
        + 0.096 * z.powi(6)
}

fn calc_f2dmuf(cp: &ControlPt, distance: f64, dmax: f64, b: f64) -> f64 {
    let d = distance.min(dmax);
    let cd = calc_cd(d, dmax);
    let c3k = calc_cd(3000.0, dmax);
    (1.0 + (cd / c3k) * (b - 1.0)) * cp.fo_f2 + (cp.fh[HR300] / 2.0) * (1.0 - (distance / dmax))
}

fn diurnal_exponent(cp: &ControlPt, month: usize) -> f64 {
    let ppt = [30.0, 30.0, 30.0, 27.5, 32.5, 35.0, 37.5, 35.0, 32.5, 30.0, 30.0, 30.0];
    let mut moddip = (cp.dip[HR100].atan2(cp.l.lat.cos().sqrt())).abs();
    if moddip > 70.0 * D2R {
        moddip = 70.0 * D2R;
    }
    let mut month = month;
    if cp.l.lat < 0.0 {
        month += 6;
        if month > 11 {
            month -= 12;
        }
    }
    let pp = ppt[month] * D2R;
    let i = if moddip > pp {
        moddip = -1.0 + 2.0 * (moddip - pp) / (70.0 * D2R - pp);
        1
    } else {
        moddip = -1.0 + 2.0 * moddip / pp;
        0
    };
    let mut p = 0.0;
    let mut sx = 1.0;
    for j in 0..7 {
        let a = if month <= 5 {
            PVAL1[(month * 2 + i) * 7 + j]
        } else {
            PVAL2[((month - 6) * 2 + i) * 7 + j]
        };
        p += a * sx;
        sx *= moddip;
    }
    p
}

fn absorption_factor(cp: &ControlPt, month: usize) -> f64 {
    let i = match month {
        6 => 5,
        7 => 4,
        8 => 6,
        9 => 7,
        10 => 8,
        11 => 0,
        m => m,
    };
    let mut x = (cp.l.lat * R2D).abs();
    if x >= 70.0 {
        x = 69.99;
    }
    x /= 2.5;
    let j = x as usize;
    x -= j as f64;
    ATNO[i * 29 + j + 1] * x + ATNO[i * 29 + j] * (1.0 - x)
}

fn penetration_factor(t: f64) -> f64 {
    let phi = if t <= 1.0 {
        if t < 0.0 {
            0.0
        } else {
            let x = (t - 0.475) / 0.475;
            let phi = (((((-0.093 * x + 0.04) * x + 0.127) * x - 0.027) * x + 0.044) * x + 0.159)
                * x
                + 0.225;
            phi.min(0.53)
        }
    } else if t <= 2.2 {
        let x = (t - 1.65) / 0.55;
        let phi =
            (((((0.043 * x - 0.07) * x - 0.027) * x + 0.034) * x + 0.054) * x - 0.049) * x + 0.375;
        phi.min(0.53)
    } else if t <= 10.0 {
        0.34 + ((10.0 - t) * 0.02) / 7.8
    } else {
        0.34
    };
    phi / 0.34
}

fn absorption_term(cp: &ControlPt, month: usize, fv: f64) -> f64 {
    let p = diurnal_exponent(cp, month);
    let chij = cp.sun.sza.min(102.0 * D2R);
    let fchij = (0.881 * chij).cos().powf(p).max(0.02);
    let mut noon = *cp;
    solar(&mut noon, month, cp.sun.lsn);
    let fnoon = (0.881 * noon.sun.sza).cos().powf(p).max(0.02);
    absorption_factor(cp, month) * penetration_factor(fv / cp.fo_e) * fchij / fnoon
}

fn season_for_lh(l: Location, month: usize) -> usize {
    let north = l.lat >= 0.0;
    match month {
        11 | 0 | 1 => {
            if north {
                WINTER
            } else {
                SUMMER
            }
        }
        5..=7 => {
            if north {
                SUMMER
            } else {
                WINTER
            }
        }
        _ => EQUINOX,
    }
}

fn find_lh(cp: &ControlPt, dh: f64, hour: i32, month: usize) -> f64 {
    let gn = geomagnetic(cp.l);
    let season = season_for_lh(cp.l, month);
    let range = if dh <= 2500.0 { 0 } else { 1 };
    let mplt = match hour {
        1..=3 => 0,
        4..=6 => 1,
        7..=9 => 2,
        10..=12 => 3,
        13..=15 => 4,
        16..=18 => 5,
        19..=21 => 6,
        h if !(1..22).contains(&h) => 7,
        _ => 0,
    };
    let lat = gn.lat.abs();
    let gmlat = if 77.5 * D2R <= lat {
        0
    } else if 72.5 * D2R <= lat {
        1
    } else if 67.5 * D2R <= lat {
        2
    } else if 62.5 * D2R <= lat {
        3
    } else if 57.5 * D2R <= lat {
        4
    } else if 52.5 * D2R <= lat {
        5
    } else if 47.5 * D2R <= lat {
        6
    } else if 42.5 * D2R <= lat {
        7
    } else {
        return 0.0;
    };
    LH[((range * 3 + season) * 8 + gmlat) * 8 + mplt]
}

fn winter_anomaly(lat: f64, month: usize) -> f64 {
    let ins = if lat < 0.0 { 1 } else { 0 };
    let lat = lat.abs();
    if (0.0..=30.0 * D2R).contains(&lat) || lat >= 90.0 * D2R {
        0.0
    } else if lat < 60.0 * D2R {
        AW[month * 2 + ins] * (lat * R2D - 30.0) / 30.0
    } else {
        AW[month * 2 + ins] * (90.0 - lat * R2D) / 30.0
    }
}

fn mirror_height(p: &Path, cp: &ControlPt, d: f64) -> f64 {
    let x = cp.fo_f2 / cp.fo_e;
    let y = x.max(1.8);
    let delta_m = (0.18 / (y - 1.4)) + (0.096 * (p.ssn.min(160) as f64 - 25.0) / 150.0);
    let xr = p.frequency / cp.fo_f2;
    let h = (1490.0 / (cp.m3k + delta_m)) - 316.0;
    if x > 3.33 && xr >= 1.0 {
        let e1 = -0.09707 * xr.powi(3) + 0.6870 * xr * xr - 0.7506 * xr + 0.6;
        let f1 = if xr <= 1.71 {
            -1.862 * xr.powi(4) + 12.95 * xr.powi(3) - 32.03 * xr * xr + 33.50 * xr - 10.91
        } else {
            1.21 + 0.2 * xr
        };
        let g = if xr <= 3.7 {
            -2.102 * xr.powi(4) + 19.50 * xr.powi(3) - 63.15 * xr * xr - 44.73
        } else {
            19.25
        };
        let ds = 160.0 + (h + 43.0) * g;
        let a = (d - ds) / (h + 140.0);
        let a1 = 140.0 + (h - 47.0) * e1;
        let b1 = 150.0 + (h - 17.0) * f1 - a1;
        let hh = if b1 >= 0.0 && a >= 0.0 { a1 + b1 * 2.4f64.powf(-a) } else { a1 + b1 };
        hh.min(800.0)
    } else if x > 3.33 && xr < 1.0 {
        let z = xr.max(0.1);
        let e2 = 0.1906 * z * z + 0.00583 * z + 0.1936;
        let a2 = 151.0 + (h - 47.0) * e2;
        let f2 = 0.645 * z * z + 0.883 * z + 0.162;
        let b2 = 141.0 + (h - 24.0) * f2 - a2;
        let df = (0.115 * d / (z * (h + 140.0))).min(0.65);
        let b = -7.535 * df.powi(4) + 15.75 * df.powi(3) - 8.834 * df * df - 0.378 * df + 1.0;
        let hh = if b2 >= 0.0 { a2 + b2 * b } else { a2 + b2 };
        hh.min(800.0)
    } else if x <= 3.33 {
        let j = -0.7126 * y.powi(3) + 5.863 * y * y - 16.13 * y + 16.07;
        let u = 8.0e-5 * (h - 80.0) * (1.0 + 11.0 * y.powf(-2.2)) + 1.2e-3 * h * y.powf(-3.6);
        (115.0 + h * j + u * d).min(800.0)
    } else {
        0.0
    }
}

impl<'a> Path<'a> {
    fn cp_params(&self, cp: &mut ControlPt) {
        ionospheric(cp, self.ctx.maps, self.hour, self.ssn);
        solar(cp, self.month, self.hour as f64);
        find_fo_e(cp, self.month, self.hour as i32, self.ssn);
        magfit(cp, 100.0);
        magfit(cp, 300.0);
    }

    fn gcp(&self, cp: &mut ControlPt, fraction: f64) {
        great_circle_point(self.l_tx, self.l_rx, cp, self.distance, fraction);
    }

    fn init(&mut self) {
        self.distance = great_circle_distance(self.l_tx, self.l_rx);
        if self.distance == 0.0 {
            self.distance = f64::EPSILON;
        }
        if self.long {
            self.distance = R0 * PI * 2.0 - self.distance;
        }
        let mut mp = ControlPt::default();
        self.gcp(&mut mp, 0.5);
        self.cp_params(&mut mp);
        self.cp[MP] = mp;
        if self.distance >= 2000.0 {
            let mut r1k = ControlPt::default();
            let mut t1k = ControlPt::default();
            self.gcp(&mut r1k, (self.distance - 1000.0) / self.distance);
            self.gcp(&mut t1k, 1000.0 / self.distance);
            self.cp_params(&mut t1k);
            self.cp_params(&mut r1k);
            self.cp[T1K] = t1k;
            self.cp[R1K] = r1k;
        }
        self.season = what_season(self.cp[MP].l, self.month);
    }

    fn muf_basic(&mut self) {
        if self.distance > 9000.0 {
            return;
        }
        let hr = (1490.0 / self.cp[MP].m3k - 176.0).min(500.0);
        self.cp[MP].hr = hr;
        let minele = MINELE * D2R;
        let aoi = incidence_angle(minele, hr);
        let dh = ((PI - aoi - (PI / 2.0 + minele)) * R0 * 2.0).min(4000.0);
        let mut n0 = 0;
        while n0 < MAXF2 {
            if dh > self.distance / (n0 as f64 + 1.0) {
                self.n0_f2 = n0;
                break;
            }
            n0 += 1;
        }
        if self.n0_f2 != NOLOWEST {
            self.dmax = calc_dmax(&mut self.cp[MP]).min(4000.0);
            let dmax = self.dmax;
            let d = self.distance;
            if d <= dmax {
                let b = calc_b(&mut self.cp[MP]);
                let muf = calc_f2dmuf(&self.cp[MP], d / (n0 as f64 + 1.0), dmax, b);
                self.md_f2[self.n0_f2].bmuf = muf;
                self.bmuf = muf;
            } else {
                let mut td = ControlPt::default();
                let mut rd = ControlPt::default();
                self.gcp(&mut td, 1.0 / (2.0 * (n0 as f64 + 1.0)));
                self.gcp(&mut rd, 1.0 - 1.0 / (2.0 * (n0 as f64 + 1.0)));
                self.cp_params(&mut td);
                self.cp_params(&mut rd);
                self.cp[TD02] = td;
                self.cp[RD02] = rd;
                let bt = calc_b(&mut self.cp[TD02]);
                let br = calc_b(&mut self.cp[RD02]);
                let m0 = calc_f2dmuf(&self.cp[TD02], d / (n0 as f64 + 1.0), dmax, bt);
                let m1 = calc_f2dmuf(&self.cp[RD02], d / (n0 as f64 + 1.0), dmax, br);
                self.md_f2[n0].bmuf = m0.min(m1);
                self.bmuf = self.md_f2[n0].bmuf;
            }
            for n in n0 + 1..MAXF2 {
                if d <= self.dmax {
                    let b = calc_b(&mut self.cp[MP]);
                    self.md_f2[n].bmuf = calc_f2dmuf(&self.cp[MP], d / (n as f64 + 1.0), dmax, b);
                } else {
                    let mut f = |cpi: usize, hops: f64| {
                        let dm = calc_dmax(&mut self.cp[cpi]);
                        let b = calc_b(&mut self.cp[cpi]);
                        calc_f2dmuf(&self.cp[cpi], d / hops, dm, b)
                    };
                    let mn0 = [f(TD02, n0 as f64 + 1.0), f(RD02, n0 as f64 + 1.0)];
                    let mn = [f(TD02, n as f64 + 1.0), f(RD02, n as f64 + 1.0)];
                    self.md_f2[n].bmuf = self.bmuf * (mn[0] / mn0[0]).min(mn[1] / mn0[1]);
                }
            }
        }
        if self.distance < 4000.0 {
            let hr = 110.0;
            let aoi = incidence_angle(minele, hr);
            let dh = ((PI - aoi - (PI / 2.0 + minele)) * R0 * 2.0).min(4000.0);
            let mut n0 = 0;
            while n0 < 3 {
                if dh > self.distance / (n0 as f64 + 1.0) {
                    self.n0_e = n0;
                    break;
                }
                n0 += 1;
            }
            if self.n0_e != NOLOWEST {
                for n in n0..MAXE {
                    self.md_e[n].hr = hr;
                    let dh = (self.distance / (n as f64 + 1.0)).min(4000.0);
                    let delta = elevation_angle(dh, hr);
                    let i110 = incidence_angle(delta, hr);
                    self.md_e[n].bmuf = if self.distance < 2000.0 {
                        self.cp[MP].fo_e / i110.cos()
                    } else if self.distance <= 4000.0 {
                        self.cp[R1K].fo_e.min(self.cp[T1K].fo_e) / i110.cos()
                    } else {
                        0.0
                    };
                    if self.md_e[n].bmuf != 0.0 && self.n0_e == NOLOWEST {
                        self.n0_e = n;
                    }
                }
            }
        }
        self.bmuf = match (self.n0_e != NOLOWEST, self.n0_f2 != NOLOWEST) {
            (true, true) => self.md_e[self.n0_e].bmuf.max(self.md_f2[self.n0_f2].bmuf),
            (true, false) => self.md_e[self.n0_e].bmuf,
            (false, true) => self.md_f2[self.n0_f2].bmuf,
            (false, false) => TOOBIG,
        };
    }

    fn fo_f2_var(&self, hour: f64, lat: f64, decile: usize) -> f64 {
        let lat = (lat / (5.0 * D2R)).abs();
        let mut r = lat - (lat as i32) as f64;
        let mut c = hour - (hour as i32) as f64;
        let mut lat_l = lat.floor() as i32;
        let mut lat_u = lat.ceil() as i32;
        if lat_l < 0 {
            lat_l = 18;
            r = 1.0 - r;
        }
        if lat_u > 18 {
            lat_u = 0;
        }
        let mut hour_l = hour.floor() as i32;
        let mut hour_u = hour.ceil() as i32;
        if hour_l < 0 {
            hour_l = 23;
            c = 1.0 - c;
        }
        if hour_u > 23 {
            hour_u = 0;
        }
        let ssn = if self.ssn < 50 {
            0
        } else if self.ssn <= 100 {
            1
        } else {
            2
        };
        let d = self.ctx.deciles;
        let s = self.season;
        let (hl, hu, ll, lu) = (hour_l as usize, hour_u as usize, lat_l as usize, lat_u as usize);
        bilinear(
            d.get(s, hl, ll, ssn, decile),
            d.get(s, hu, ll, ssn, decile),
            d.get(s, hl, lu, ssn, decile),
            d.get(s, hu, lu, ssn, decile),
            r,
            c,
        )
    }

    fn muf_variability(&mut self) {
        if self.distance > 9000.0 {
            return;
        }
        self.muf50 = self.bmuf;
        let f = self.frequency;
        let fprob = |m: &Mode| {
            if f < m.muf50 {
                (1.3 - (0.8 / (1.0 + ((1.0 - (f / m.muf50)) / (1.0 - m.deltal))))).min(1.0)
            } else {
                ((0.8 / (1.0 + (((f / m.muf50) - 1.0) / (m.deltau - 1.0)))) - 0.3).max(0.0)
            }
        };
        for i in 0..MAXF2 {
            if self.md_f2[i].bmuf != 0.0 {
                let dl = self.fo_f2_var(self.cp[MP].ltime, self.cp[MP].l.lat, 0);
                let du = self.fo_f2_var(self.cp[MP].ltime, self.cp[MP].l.lat, 1);
                let m = &mut self.md_f2[i];
                m.muf50 = m.bmuf;
                m.deltal = dl;
                m.deltau = du;
                m.muf10 = du * m.muf50;
                m.muf90 = dl * m.muf50;
                m.fprob = fprob(m);
            }
        }
        for m in self.md_e.iter_mut().filter(|m| m.bmuf != 0.0) {
            m.muf50 = m.bmuf;
            m.deltal = 0.95;
            m.deltau = 1.05;
            m.muf10 = m.deltau * m.muf50;
            m.muf90 = m.deltal * m.muf50;
            m.fprob = fprob(m);
        }
        let best = |ms: &[Mode], f: fn(&Mode) -> f64| {
            ms.iter().filter(|m| m.bmuf != 0.0).map(f).fold(0.0, f64::max)
        };
        self.muf90 = best(&self.md_e, |m| m.muf90).max(best(&self.md_f2, |m| m.muf90));
        self.muf10 = best(&self.md_e, |m| m.muf10).max(best(&self.md_f2, |m| m.muf10));
    }

    fn muf_operational(&mut self) {
        if self.distance > 9000.0 {
            return;
        }
        let rop = [[1.20, 1.30], [1.15, 1.25], [1.10, 1.20]];
        let mp = &self.cp[MP];
        let day = (mp.ltime < mp.sun.lss && mp.ltime > mp.sun.lsr)
            && !(mp.ltime > mp.sun.lss && mp.ltime < mp.sun.lsr);
        let time = if day { 0 } else { 1 };
        let factor = rop[self.season][time];
        for m in self.md_f2.iter_mut().filter(|m| m.bmuf != 0.0) {
            m.opmuf = m.muf50 * factor;
            m.opmuf10 = m.opmuf * m.deltau;
            m.opmuf90 = m.opmuf * m.deltal;
        }
        for m in self.md_e.iter_mut().filter(|m| m.bmuf != 0.0) {
            m.opmuf = m.bmuf;
            m.opmuf10 = m.opmuf * m.deltau;
            m.opmuf90 = m.opmuf * m.deltal;
        }
        let best = |ms: &[Mode], f: fn(&Mode) -> f64| {
            ms.iter().filter(|m| m.bmuf != 0.0).map(f).fold(0.0, f64::max)
        };
        self.opmuf = best(&self.md_e, |m| m.opmuf).max(best(&self.md_f2, |m| m.opmuf));
        self.opmuf90 = best(&self.md_e, |m| m.opmuf90).max(best(&self.md_f2, |m| m.opmuf90));
        self.opmuf10 = best(&self.md_e, |m| m.opmuf10).max(best(&self.md_f2, |m| m.opmuf10));
    }

    fn e_screening(&mut self) {
        if self.distance > 4000.0 {
            return;
        }
        let fo_e = if self.distance <= 2000.0 {
            self.cp[MP].fo_e
        } else {
            self.cp[T1K].fo_e.max(self.cp[R1K].fo_e)
        };
        if self.n0_f2 == NOLOWEST {
            return;
        }
        for k in self.n0_f2..MAXF2 {
            let dh = self.distance / (k as f64 + 1.0);
            let hr = if self.distance <= self.dmax {
                mirror_height(self, &self.cp[MP], dh)
            } else {
                (mirror_height(self, &self.cp[TD02], dh)
                    + mirror_height(self, &self.cp[MP], dh)
                    + mirror_height(self, &self.cp[RD02], dh))
                    / 3.0
            };
            self.md_f2[k].hr = hr;
            let deltaf = elevation_angle(dh, hr);
            let i = incidence_angle(deltaf, 110.0);
            self.md_f2[k].fs = (1.05 * fo_e) / i.cos();
        }
    }

    fn gain(&self, ant: &Pattern, delta: f64, tx_to_rx: bool) -> f64 {
        let delta = delta * R2D;
        let b = if tx_to_rx {
            bearing(self.l_tx, self.l_rx, self.long) * R2D
        } else {
            bearing(self.l_rx, self.l_tx, self.long) * R2D
        };
        let (du, dl) = (delta.ceil() as i64, delta.floor() as i64);
        let (br, bl) = ((b.ceil() as i64) % 360, (b.floor() as i64) % 360);
        let at = |az: i64, el: i64| ant.at(az.max(0) as usize, el.max(0) as usize);
        let r = delta - (delta as i64) as f64;
        let c = b - (b as i64) as f64;
        bilinear(at(bl, dl), at(br, dl), at(bl, du), at(br, du), r, c)
    }

    fn gain08(&self, ant: &Pattern, tx_to_rx: bool) -> (f64, f64) {
        let mut best = TINYDB;
        let mut ele = 2.0 * PI;
        for i in 0..9 {
            let g = self.gain(ant, i as f64 * D2R, tx_to_rx);
            if g > best {
                best = g;
                ele = i as f64 * D2R;
            }
        }
        (best, ele)
    }

    fn penetration_points(&self, noh: usize, hr: f64, fv: f64) -> f64 {
        let dh = self.distance / (noh as f64 + 1.0);
        let delta = elevation_angle(dh, hr);
        let aoi90 = incidence_angle(delta, 90.0);
        let dh90 = R0 * (PI / 2.0 - delta - aoi90);
        let mut sum = 0.0;
        for i in 0..=noh {
            for frac in [(i as f64 * dh + dh90), ((i as f64 + 1.0) * dh - dh90)] {
                let mut p = ControlPt::default();
                self.gcp(&mut p, frac / self.distance);
                self.cp_params(&mut p);
                p.hr = 90.0;
                sum += absorption_term(&p, self.month, fv);
            }
        }
        sum / (2.0 * (noh as f64 + 1.0))
    }

    fn smallest_cp_fo_f2(&self) -> usize {
        let mut idx = [0usize, 1, 2, 3, 4];
        let mut temp = 0usize;
        for i in 0..5 {
            for j in 0..5 {
                if self.cp[idx[i]].fo_f2 > self.cp[idx[j]].fo_f2 {
                    temp = idx[i];
                    idx[i] = idx[j];
                    idx[j] = temp;
                }
            }
        }
        for v in idx {
            if v != 0 {
                temp = v;
            }
        }
        temp
    }

    fn f2_active(&self, n: usize) -> bool {
        let f = self.frequency;
        self.md_f2[n].fs < f
            && ((n == self.n0_f2 && self.distance / (self.n0_f2 as f64 + 1.0) <= self.dmax)
                || (n != self.n0_f2 && self.md_f2[n].bmuf != 0.0))
    }

    fn fl_of(&self, cps: &[usize]) -> f64 {
        cps.iter().map(|&i| (self.cp[i].fh[HR100] * self.cp[i].dip[HR100].sin()).abs()).sum::<f64>()
            / cps.len() as f64
    }

    fn lh_of(&self, cps: &[usize], dh: f64, mpltime: i32) -> f64 {
        cps.iter().map(|&i| find_lh(&self.cp[i], dh, mpltime, self.month)).sum::<f64>()
            / cps.len() as f64
    }

    fn field_short(&mut self) {
        if self.distance > 9000.0 {
            return;
        }
        let ssn = self.ssn.min(MAXSSN) as f64;
        let hr_e = 110.0;
        let hr_f2 = if self.distance > self.dmax {
            ((1490.0 / self.cp[self.smallest_cp_fo_f2()].m3k) - 176.0).min(500.0)
        } else {
            self.cp[MP].hr
        };
        let tz = (self.cp[MP].l.lng / (15.0 * D2R)) as i32;
        let mpltime = ((self.cp[MP].ltime + tz as f64) % 24.0) as i32;
        let f = self.frequency;
        if self.n0_e != NOLOWEST {
            for n in self.n0_e..MAXE {
                let ok = (n == self.n0_e && self.distance / (self.n0_e as f64 + 1.0) <= 2000.0)
                    || (n > self.n0_e && self.md_e[n].bmuf != 0.0);
                if !ok {
                    break;
                }
                let nf = n as f64 + 1.0;
                let delta = elevation_angle(self.distance / nf, hr_e);
                self.md_e[n].ele = delta;
                let aoi110 = incidence_angle(delta, hr_e);
                let fv = f * aoi110.cos();
                let dh = self.distance / nf;
                let psi = dh / (2.0 * R0);
                self.ptick = (2.0 * R0 * (psi.sin() / (delta + psi).cos())).abs() * nf;
                let at = self.penetration_points(n, hr_e, fv);
                let (fl, lh) = if self.distance <= 2000.0 {
                    (self.fl_of(&[MP]), self.lh_of(&[MP], dh, mpltime))
                } else {
                    (self.fl_of(&[MP, T1K, R1K]), self.lh_of(&[MP, T1K, R1K], dh, mpltime))
                };
                let li = (nf * (1.0 + 0.0067 * ssn) * at) / ((f + fl).powi(2) * aoi110.cos());
                let lm = if f <= self.md_e[n].bmuf {
                    0.0
                } else {
                    (46.0 * ((f / self.md_e[n].bmuf) - 1.0).powf(0.5) + 5.0).min(58.0)
                };
                let lg = 2.0 * (nf - 1.0);
                let lb =
                    32.45 + 20.0 * f.log10() + 20.0 * self.ptick.log10() + li + lm + lg + lh + 9.14;
                self.md_e[n].lb = lb;
                let gt = self.gain(self.ctx.tx_ant, delta, true);
                self.md_e[n].ew = 136.6 + self.txpower + gt + 20.0 * f.log10() - lb;
            }
        }
        if self.n0_f2 != NOLOWEST {
            for n in self.n0_f2..MAXF2 {
                if !self.f2_active(n) {
                    continue;
                }
                let nf = n as f64 + 1.0;
                let delta = elevation_angle(self.distance / nf, hr_f2);
                self.md_f2[n].ele = delta;
                let aoi110 = incidence_angle(delta, 110.0);
                let fv = f * aoi110.cos();
                let dh = self.distance / nf;
                let psi = dh / (2.0 * R0);
                self.ptick = (2.0 * R0 * (psi.sin() / (delta + psi).cos())).abs() * nf;
                let at = self.penetration_points(n, hr_f2, fv);
                let cps: &[usize] = if self.distance <= 2000.0 {
                    &[MP]
                } else if self.distance <= self.dmax {
                    &[MP, T1K, R1K]
                } else {
                    &[MP, T1K, R1K, TD02, RD02]
                };
                let fl = self.fl_of(cps);
                let lh = self.lh_of(cps, dh, mpltime);
                let li = (nf * (1.0 + 0.0067 * ssn) * at) / ((f + fl).powi(2) * aoi110.cos());
                let bm = self.md_f2[n].bmuf;
                let lm = if f <= bm {
                    0.0
                } else if self.distance <= 3000.0 {
                    (36.0 * ((f / bm) - 1.0).powf(0.5) + 5.0).min(60.0)
                } else {
                    (70.0 * (f / bm - 1.0) + 8.0).min(80.0)
                };
                let lg = 2.0 * (nf - 1.0);
                let lb =
                    32.45 + 20.0 * f.log10() + 20.0 * self.ptick.log10() + li + lm + lg + lh + 9.14;
                self.md_f2[n].lb = lb;
                let gt = self.gain(self.ctx.tx_ant, delta, true);
                self.md_f2[n].ew = 136.6 + self.txpower + gt + 20.0 * f.log10() - lb;
            }
        }
        self.es = TINYDB;
        let mut etw = 0.0;
        if self.n0_e != NOLOWEST {
            for n in self.n0_e..MAXE {
                if (n == self.n0_e && self.distance / (self.n0_e as f64 + 1.0) <= 2000.0)
                    || (n != self.n0_e && self.md_e[n].bmuf != 0.0)
                {
                    etw += 10f64.powf(self.md_e[n].ew / 10.0);
                }
            }
        }
        if self.n0_f2 != NOLOWEST {
            for n in self.n0_f2..MAXF2 {
                if self.f2_active(n) {
                    etw += 10f64.powf(self.md_f2[n].ew / 10.0);
                }
            }
        }
        if etw != 0.0 {
            self.es = 10.0 * etw.log10();
        }
    }

    fn field_long(&mut self) {
        if self.distance < 7000.0 {
            return;
        }
        let hr = 300.0;
        let mut n = 0;
        while self.distance / (n as f64 + 1.0) > 3000.0 {
            n += 1;
        }
        let nl = n;
        let dl = self.distance / (nl as f64 + 1.0);
        let delta_l = elevation_angle(dl, hr);
        let mut n = 0;
        while self.distance / (n as f64 + 1.0) > 4000.0 {
            n += 1;
        }
        let mut nm = n;
        let mut dm = self.distance / (nm as f64 + 1.0);
        let mut delta_m = elevation_angle(dm, hr);
        if delta_m < MINELE * D2R {
            nm += 1;
            dm = self.distance / (nm as f64 + 1.0);
            delta_m = elevation_angle(dm, hr);
        }
        let i90 = incidence_angle(delta_l, 90.0);
        let dh90 = R0 * (PI / 2.0 - delta_l - i90);
        let saved = self.hour;
        let mut cps = vec![[ControlPt::default(); 24]; 2 * (nl + 1)];
        let mut tdm = [ControlPt::default(); 24];
        let mut rdm = [ControlPt::default(); 24];
        for j in 0..24 {
            self.hour = j;
            for i in 0..=nl {
                let mut a = ControlPt::default();
                self.gcp(&mut a, (i as f64 * dl + dh90) / self.distance);
                self.cp_params(&mut a);
                a.hr = 90.0;
                cps[2 * i][j] = a;
                let mut b = ControlPt::default();
                self.gcp(&mut b, ((i as f64 + 1.0) * dl - dh90) / self.distance);
                self.cp_params(&mut b);
                b.hr = 90.0;
                cps[2 * i + 1][j] = b;
            }
            let mut t = ControlPt::default();
            let mut r = ControlPt::default();
            self.gcp(&mut t, 1.0 / (2.0 * (nm as f64 + 1.0)));
            self.gcp(&mut r, 1.0 - 1.0 / (2.0 * (nm as f64 + 1.0)));
            self.cp_params(&mut t);
            self.cp_params(&mut r);
            for c in [&mut t, &mut r] {
                c.x = 0.0;
                c.fo_e = 0.0;
                c.hr = 300.0;
            }
            tdm[j] = t;
            rdm[j] = r;
        }
        self.hour = saved;
        let psi = dm / (2.0 * R0);
        self.ptick = (2.0 * R0 * (psi.sin() / (delta_m + psi).cos())).abs() * (nm as f64 + 1.0);
        self.e0 = 139.6 - 20.0 * self.ptick.log10();
        self.gtl = self.gain08(self.ctx.tx_ant, true).0;
        let d = self.distance;
        self.gap = (10.0 * (d / (R0 * (d / R0).sin().abs())).log10()).min(15.0);
        self.ly = -0.17;
        self.fh = (tdm[self.hour].fh[HR300] + rdm[self.hour].fh[HR300]) / 2.0;
        self.find_mufs_and_fm(&tdm, &rdm, dm);
        self.find_fl(&cps, nl, i90);
        let f = self.frequency;
        let (fl, fh, fm) = (self.fl, self.fh, self.fm);
        let mut etl =
            ((fl + fh).powi(2) / (f + fh).powi(2)) + ((f + fh).powi(2) / (fm + fh).powi(2));
        etl *= (fm + fh).powi(2) / ((fm + fh).powi(2) + (fl + fh).powi(2));
        self.el = self.e0 * (1.0 - etl) - 30.0 + self.txpower + self.gtl + self.gap - self.ly;
        if self.distance > 9000.0 {
            self.ele = delta_m;
            self.cp[RD02] = rdm[self.hour];
            self.cp[TD02] = tdm[self.hour];
            self.cp[T1K] = cps[0][self.hour];
            self.cp[R1K] = cps[2 * nl][self.hour];
            self.dmax = 4000.0;
        }
    }

    fn find_mufs_and_fm(&mut self, tdm: &[ControlPt; 24], rdm: &[ControlPt; 24], dm: f64) {
        let coef = [
            -2.40074637494790e-24,
            25.8520201885984e-21,
            -92.4986988833091e-18,
            102.342990689362e-15,
            22.0776941764705e-12,
            87.4376851991085e-9,
            29.1996868566837e-6,
        ];
        let fd = coef.iter().skip(1).fold(coef[0] * dm, |acc, c| (acc + c) * dm);
        let noon_of =
            |c: &ControlPt| (((12.0 - c.l.lng / (15.0 * D2R)) as i32 - 1 + 24) % 24) as usize;
        let noon = [noon_of(&tdm[1]), noon_of(&rdm[1])];
        let mut fbm = [[0.0; 24]; 2];
        let mut fbm_min = [100.0f64; 2];
        for t in 0..24 {
            for (n, c) in [&tdm[t], &rdm[t]].iter().enumerate() {
                let f4 = 1.1 * c.fo_f2 * c.m3k;
                let fz = c.fo_f2 + 0.5 * c.fh[HR300];
                fbm[n][t] = fz + (f4 - fz) * fd;
                fbm_min[n] = fbm[n][t].min(fbm_min[n]);
            }
        }
        let mut a = bearing(self.cp[MP].l, self.l_rx, false);
        if a > PI {
            a -= PI;
        }
        if a >= PI / 2.0 {
            a -= PI / 2.0;
        } else {
            a = PI / 2.0 - a;
        }
        let ew = a / (PI / 2.0);
        let iw = 0.1 * (1.0 - ew) + 0.2 * ew;
        let iy = 0.6 * (1.0 - ew) + 0.4 * ew;
        let ix = 1.2 * (1.0 - ew) + 0.2 * ew;
        let h = self.hour;
        for n in 0..2 {
            self.k[n] = 1.2
                + iw * (fbm[n][h] / fbm[n][noon[n]])
                + ix * ((fbm[n][noon[n]] / fbm[n][h]).powf(1.0 / 3.0) - 1.0)
                + iy * (fbm_min[n] / fbm[n][noon[n]]).powi(2);
        }
        self.fm = (self.k[0] * fbm[0][h]).min(self.k[1] * fbm[1][h]);
        if self.distance > 9000.0 {
            let smaller = if fbm[0][h] < fbm[1][h] {
                self.bmuf = fbm[0][h];
                &tdm[h]
            } else {
                self.bmuf = fbm[1][h];
                &rdm[h]
            };
            let dl = self.fo_f2_var(smaller.ltime, smaller.l.lat, 0);
            let du = self.fo_f2_var(smaller.ltime, smaller.l.lat, 1);
            self.muf50 = self.bmuf;
            self.muf10 = self.muf50 * du;
            self.muf90 = self.muf50 * dl;
            self.opmuf = self.fm;
            self.opmuf10 = self.opmuf * du;
            self.opmuf90 = self.opmuf * dl;
        }
    }

    fn find_fl(&mut self, cps: &[[ControlPt; 24]], hops: usize, i90: f64) {
        let mut sum = [0.0; 24];
        for (t, s) in sum.iter_mut().enumerate() {
            for c in cps.iter().take(2 * (hops + 1)) {
                let chi = c[t].sun.sza;
                if chi > 0.0 && chi < PI / 2.0 {
                    *s += chi.cos().sqrt();
                }
            }
        }
        let aw = winter_anomaly(self.cp[MP].l.lat, self.month);
        let fln = (self.distance / 3000.0).sqrt();
        let mut fl = [0.0; 24];
        for i in 0..24 {
            fl[i] = ((5.3
                * (((1.0 + 0.009 * self.ssn as f64) * sum[i])
                    / (i90.cos() * (9.5e6 / self.ptick).ln()))
                .sqrt()
                - self.fh)
                * (aw + 1.0))
                .max(fln);
        }
        let mut tr: Option<usize> = None;
        for now in 0..24 {
            let prev = (now + 23) % 24;
            if tr.is_none() && fl[prev] >= 2.0 * fln && fl[now] <= 2.0 * fln {
                tr = Some(now);
                let dt = (2.0 * fln - fl[now]) / (fl[prev] - fl[now]);
                fl[now] = 0.7945 * fl[prev] * (dt * (1.0 - 0.7945) + 0.7945);
            }
        }
        if let Some(tr) = tr {
            for i in 1..4 {
                let now = (tr + i) % 24;
                let prev = (now + 23) % 24;
                fl[now] = (fl[prev] * 0.7945).max(fl[now]);
            }
        }
        self.fl = fl[(self.hour + 1) % 24];
    }

    fn between(&mut self) {
        if 7000.0 < self.distance && self.distance < 9000.0 {
            let xl = 10f64.powf(self.el / 100.0);
            let xs = 10f64.powf(self.es / 100.0);
            let xi = xs + ((self.distance - 7000.0) / 2000.0) * (xl - xs);
            self.ei = 100.0 * xi.log10();
            let hops = self.n0_f2 as f64 + 1.0;
            let mut muf = |cpi: usize| {
                let b = calc_b(&mut self.cp[cpi]);
                let dmax = calc_dmax(&mut self.cp[cpi]).min(4000.0);
                calc_f2dmuf(&self.cp[cpi], self.distance / hops, dmax, b)
            };
            let a = muf(TD02);
            let b = muf(RD02);
            self.bmuf = a.min(b);
        }
    }

    fn receiver_power(&mut self) {
        let f = self.frequency;
        if self.distance <= 7000.0 {
            let mut sum = 0.0;
            let mut best = TINYDB;
            if self.n0_e != NOLOWEST {
                for i in self.n0_e..MAXE {
                    if (i == self.n0_e && self.distance / (self.n0_e as f64 + 1.0) <= 2000.0)
                        || (i != self.n0_e && self.md_e[i].bmuf != 0.0)
                    {
                        let g = self.gain(self.ctx.rx_ant, self.md_e[i].ele, false);
                        self.md_e[i].grw = g;
                        self.md_e[i].prw = self.md_e[i].ew + g - 20.0 * f.log10() - 107.2;
                        if best < self.md_e[i].prw {
                            best = self.md_e[i].prw;
                            self.dm = Some(i);
                        }
                        sum += 10f64.powf(self.md_e[i].prw / 10.0);
                    }
                }
            }
            if self.n0_f2 != NOLOWEST {
                for i in self.n0_f2..MAXF2 {
                    if self.f2_active(i) {
                        let g = self.gain(self.ctx.rx_ant, self.md_f2[i].ele, false);
                        self.md_f2[i].grw = g;
                        self.md_f2[i].prw = self.md_f2[i].ew + g - 20.0 * f.log10() - 107.2;
                        if best < self.md_f2[i].prw {
                            best = self.md_f2[i].prw;
                            self.dm = Some(i + 3);
                        }
                        sum += 10f64.powf(self.md_f2[i].prw / 10.0);
                    }
                }
            }
            if sum != 0.0 && self.dm.is_some() {
                self.pr = 10.0 * sum.log10();
                let m = match self.dm {
                    Some(i) if i < 3 => self.md_e[i],
                    Some(i) => self.md_f2[i - 3],
                    None => Mode::default(),
                };
                self.grw = m.grw;
                self.ele = m.ele;
            } else {
                self.pr = TINYDB;
            }
            self.ep = self.es;
        } else {
            let (g, ele) = self.gain08(self.ctx.rx_ant, false);
            let e = if self.distance < 9000.0 { self.ei } else { self.el };
            self.pr = e + g - 20.0 * f.log10() - 107.2;
            self.grw = g;
            self.ep = e;
            self.ele = ele;
        }
    }
}

pub fn run(input: &Input, ctx: &Context) -> Output {
    let mut p = Path {
        ctx,
        month: input.month,
        hour: input.hour,
        ssn: input.ssn,
        frequency: input.frequency,
        txpower: input.txpower,
        l_tx: input.tx,
        l_rx: input.rx,
        long: input.long_path,
        season: 99,
        distance: 999999.9,
        ptick: 0.0,
        dmax: 999999.9,
        ele: 2.0 * PI,
        bmuf: 99.9,
        muf50: 99.9,
        muf90: 99.9,
        muf10: 99.9,
        opmuf: 99.9,
        opmuf90: 99.9,
        opmuf10: 99.9,
        n0_f2: NOLOWEST,
        n0_e: NOLOWEST,
        es: TINYDB,
        el: TINYDB,
        ei: TINYDB,
        ep: TINYDB,
        pr: TINYDB,
        e0: TINYDB,
        gap: TINYDB,
        ly: 0.0,
        fm: 0.0,
        fl: 0.0,
        fh: 0.0,
        gtl: TINYDB,
        k: [0.0; 2],
        grw: TINYDB,
        cp: [ControlPt::default(); 5],
        md_f2: [Mode::default(); MAXF2],
        md_e: [Mode::default(); MAXE],
        dm: None,
    };
    p.init();
    p.muf_basic();
    p.muf_variability();
    p.muf_operational();
    p.e_screening();
    p.field_short();
    p.field_long();
    p.between();
    p.receiver_power();
    let noise = p372::noise(
        ctx.noise,
        input.man_made,
        input.hour as i32,
        input.rx.lng,
        input.rx.lat,
        input.frequency,
    );
    let total = 10f64.powf(noise.fa_a / 10.0)
        + 10f64.powf(noise.fa_m / 10.0)
        + 10f64.powf(noise.fa_g / 10.0);
    let snr = p.pr - 10.0 * total.log10() - 10.0 * input.bw.log10() + 204.0;
    let mut gt60 = false;
    if p.distance > 2000.0 {
        for i in [MP, T1K, R1K] {
            if geomagnetic(p.cp[i].l).lat >= 60.0 * D2R {
                gt60 = true;
            }
        }
    }
    let ratio = input.frequency / p.bmuf;
    let bucket = if ratio <= 0.8 {
        0
    } else if ratio <= 1.0 {
        1
    } else if ratio <= 1.2 {
        2
    } else if ratio <= 1.4 {
        3
    } else if ratio <= 1.6 {
        4
    } else if ratio <= 1.8 {
        5
    } else if ratio <= 2.0 {
        6
    } else if ratio <= 3.0 {
        7
    } else if ratio <= 4.0 {
        8
    } else {
        9
    };
    let ld = [
        [8.0, 12.0, 13.0, 10.0, 8.0, 8.0, 8.0, 7.0, 6.0, 5.0],
        [11.0, 16.0, 17.0, 13.0, 11.0, 11.0, 11.0, 9.0, 8.0, 7.0],
    ];
    let ud = [
        [6.0, 8.0, 12.0, 13.0, 12.0, 9.0, 9.0, 8.0, 7.0, 7.0],
        [9.0, 11.0, 12.0, 13.0, 12.0, 9.0, 9.0, 8.0, 7.0, 7.0],
    ];
    let g = gt60 as usize;
    let (dl_sd, du_sd) = (ld[g][bucket], ud[g][bucket]);
    let (du_sh, dl_sh) = (5.0, 8.0);
    let x = total;
    let y = 10f64.powf((noise.fa_a - noise.dl_a) / 10.0)
        + 10f64.powf((noise.fa_m - noise.dl_m) / 10.0)
        + 10f64.powf((noise.fa_g - noise.dl_g) / 10.0);
    let du_sn = ((10.0 * (x / y).log10()).powi(2) + du_sd * du_sd + du_sh * du_sh).sqrt();
    let y = 10f64.powf((noise.fa_a + noise.du_a) / 10.0)
        + 10f64.powf((noise.fa_m + noise.du_m) / 10.0)
        + 10f64.powf((noise.fa_g + noise.du_g) / 10.0);
    let dl_sn = ((10.0 * (y / x).log10()).powi(2) + dl_sd * dl_sd + dl_sh * dl_sh).sqrt();
    let bcr = if snr >= input.snrr {
        (130.0 - 80.0 / (1.0 + ((snr - input.snrr) / dl_sn))).min(100.0)
    } else {
        (80.0 / (1.0 + ((input.snrr - snr) / du_sn)) - 30.0).max(0.0)
    };
    let xp = input.snrxxp.clamp(1, 99);
    let snrxx = if xp < 50 {
        snr + du_sn * NORM[(50 - xp) as usize] / NORM[40]
    } else {
        snr - dl_sn * NORM[(xp - 50) as usize] / NORM[40]
    };
    Output {
        distance: p.distance,
        dmax: p.dmax,
        ptick: p.ptick,
        ele: p.ele,
        bmuf: p.bmuf,
        muf50: p.muf50,
        muf90: p.muf90,
        muf10: p.muf10,
        opmuf: p.opmuf,
        opmuf90: p.opmuf90,
        opmuf10: p.opmuf10,
        n0_f2: (p.n0_f2 != NOLOWEST).then_some(p.n0_f2),
        n0_e: (p.n0_e != NOLOWEST).then_some(p.n0_e),
        ep: p.ep,
        es: p.es,
        el: p.el,
        pr: p.pr,
        grw: p.grw,
        noise,
        snr,
        du_sn,
        dl_sn,
        snrxx,
        bcr,
        f2: p.md_f2,
        e: p.md_e,
        dominant: p.dm,
    }
}
