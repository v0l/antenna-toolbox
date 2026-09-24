use crate::{D2R, PI, TINYDB};

const NOISE: &str = include_str!("../data/noise.txt");

#[derive(Clone, Debug)]
pub struct Coefficients {
    fakp: Vec<f64>,
    fakabp: Vec<f64>,
    dud: Vec<f64>,
    fam: Vec<f64>,
}

impl Coefficients {
    pub fn month(month: usize) -> Coefficients {
        let line = NOISE.lines().nth(month.min(11)).unwrap_or_default();
        let v: Vec<f64> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        Coefficients {
            fakp: v[0..2784].to_vec(),
            fakabp: v[2784..2796].to_vec(),
            dud: v[2796..3096].to_vec(),
            fam: v[3096..3264].to_vec(),
        }
    }

    fn fakp(&self, i: usize, j: usize, k: usize) -> f64 {
        self.fakp[16 * 29 * i + 29 * j + k]
    }

    fn fakabp(&self, j: usize, k: usize) -> f64 {
        self.fakabp[2 * j + k]
    }

    fn dud(&self, i: usize, j: usize, k: usize) -> f64 {
        self.dud[5 * 12 * i + 5 * j + k]
    }

    fn fam(&self, j: usize, k: usize) -> f64 {
        self.fam[14 * j + k]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ManMade {
    City,
    Residential,
    Rural,
    QuietRural,
    Noisy,
    Quiet,
    Custom(f64),
}

impl ManMade {
    pub const ALL: [ManMade; 6] = [
        ManMade::City,
        ManMade::Residential,
        ManMade::Rural,
        ManMade::QuietRural,
        ManMade::Noisy,
        ManMade::Quiet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ManMade::City => "city",
            ManMade::Residential => "residential",
            ManMade::Rural => "rural",
            ManMade::QuietRural => "quiet rural",
            ManMade::Noisy => "noisy",
            ManMade::Quiet => "quiet",
            ManMade::Custom(_) => "custom",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Noise {
    pub fa_a: f64,
    pub du_a: f64,
    pub dl_a: f64,
    pub fa_m: f64,
    pub du_m: f64,
    pub dl_m: f64,
    pub fa_g: f64,
    pub du_g: f64,
    pub dl_g: f64,
    pub du_t: f64,
    pub dl_t: f64,
    pub fam_t: f64,
}

impl Default for Noise {
    fn default() -> Self {
        Noise {
            fa_a: TINYDB,
            du_a: TINYDB,
            dl_a: TINYDB,
            fa_m: TINYDB,
            du_m: TINYDB,
            dl_m: TINYDB,
            fa_g: TINYDB,
            du_g: TINYDB,
            dl_g: TINYDB,
            du_t: TINYDB,
            dl_t: TINYDB,
            fam_t: TINYDB,
        }
    }
}

struct FamStats {
    tmblk: usize,
    fa: f64,
    du: f64,
    dl: f64,
}

fn fam_parameters(c: &Coefficients, tmblk: usize, lng: f64, lat: f64, frequency: f64) -> FamStats {
    let q = if lng < 0.0 { (lng + 2.0 * PI) / 2.0 } else { lng / 2.0 };
    let mut zz = [0.0; 29];
    for (j, z) in zz.iter_mut().enumerate() {
        let mut r = 0.0;
        for k in 0..15 {
            r += ((k as f64 + 1.0) * q).sin() * c.fakp(tmblk, k, j);
        }
        *z = r + c.fakp(tmblk, 15, j);
    }
    let q = lat + PI / 2.0;
    let mut r = 0.0;
    for (j, z) in zz.iter().enumerate() {
        r += ((j as f64 + 1.0) * q).sin() * z;
    }
    let fam1 = r + c.fakabp(tmblk, 0) + c.fakabp(tmblk, 1) * q;
    let i = if lat < 0.0 { tmblk + 6 } else { tmblk };
    let u = [-0.75, (8.0 * 2f64.powf(frequency.log10()) - 11.0) / 4.0];
    let (mut cz, mut pz, mut px) = (0.0, 0.0, 0.0);
    for (k, uk) in u.iter().enumerate() {
        pz = uk * c.fam(i, 0) + c.fam(i, 1);
        px = uk * c.fam(i, 7) + c.fam(i, 8);
        for j in 2..7 {
            pz = uk * pz + c.fam(i, j);
            px = uk * px + c.fam(i, j + 7);
        }
        if k == 0 {
            cz = fam1 * (2.0 - pz) - px;
        }
    }
    let fa = cz * pz + px;
    let mut x = frequency.log10();
    if frequency > 20.0 {
        x = 20f64.log10();
    }
    let mut v = [0.0; 5];
    for (j, vj) in v.iter_mut().enumerate() {
        if j == 4 && frequency > 10.0 {
            x = 1.0;
        }
        let mut y = c.dud(j, i, 0);
        for k in 1..5 {
            y = y * x + c.dud(j, i, k);
        }
        *vj = y;
    }
    FamStats { tmblk, fa, du: v[0], dl: v[1] }
}

fn atmospheric(c: &Coefficients, n: &mut Noise, hour: i32, lng: f64, lat: f64, frequency: f64) {
    let mut lrxmt = hour + (lng / (15.0 * D2R)) as i32;
    if lrxmt < 0 {
        lrxmt += 24;
    } else if lrxmt > 23 {
        lrxmt -= 24;
    }
    let now_blk = ((lrxmt / 4) % 6) as usize;
    let now = fam_parameters(c, now_blk, lng, lat, frequency);
    let adj = fam_parameters(c, (now.tmblk + 1) % 6, lng, lat, frequency);
    let slp = (lrxmt as f64 % 4.0) / 4.0;
    let mix = |a: f64, b: f64| {
        10.0 * (10f64.powf(a / 10.0) + (10f64.powf(b / 10.0) - 10f64.powf(a / 10.0)) * slp).log10()
    };
    n.fa_a = mix(now.fa, adj.fa);
    n.du_a = mix(now.du, adj.du);
    n.dl_a = mix(now.dl, adj.dl);
}

fn man_made(n: &mut Noise, kind: ManMade, frequency: f64) {
    let (c, d) = match kind {
        ManMade::City => {
            n.du_m = 11.0;
            n.dl_m = 6.7;
            (76.8, 27.7)
        }
        ManMade::Residential => {
            n.du_m = 10.6;
            n.dl_m = 5.3;
            (72.5, 27.7)
        }
        ManMade::Rural => {
            n.du_m = 9.2;
            n.dl_m = 4.6;
            (67.2, 27.7)
        }
        ManMade::QuietRural => {
            n.du_m = 9.2;
            n.dl_m = 4.6;
            (53.6, 28.6)
        }
        ManMade::Quiet => {
            n.du_m = 9.2;
            n.dl_m = 4.6;
            (65.2, 29.1)
        }
        ManMade::Noisy => {
            n.du_m = 11.0;
            n.dl_m = 6.7;
            (83.2, 37.5)
        }
        ManMade::Custom(v) => {
            n.dl_m = 11.0;
            n.du_m = 6.7;
            (-v + 204.0, 0.0)
        }
    };
    n.fa_m = c - d * frequency.log10();
}

fn total(fa: [f64; 3], d: [f64; 3]) -> (f64, f64) {
    let c = 10.0 / 10f64.ln();
    let sigma = [d[0] / 1.282, 1.56, d[2] / 1.282];
    let term = |f: f64, s: f64| ((f / c) + s * s / (2.0 * c * c)).exp();
    let alpha: f64 = (0..3).map(|i| term(fa[i], sigma[i])).sum();
    let beta: f64 =
        (0..3).map(|i| term(fa[i], sigma[i]).powi(2) * ((sigma[i] / c).powi(2).exp() - 1.0)).sum();
    let gamma: f64 = fa.iter().map(|f| (f / c).exp()).sum();
    let sigma_t = if d.iter().any(|v| *v > 12.0) {
        c * (2.0 * (alpha / gamma).ln()).sqrt()
    } else {
        c * (1.0 + beta / alpha.powi(2)).ln().sqrt()
    };
    (c * (alpha.ln() - sigma_t.powi(2) / (2.0 * c * c)), 1.282 * sigma_t)
}

pub fn noise(
    c: &Coefficients,
    kind: ManMade,
    hour: i32,
    lng: f64,
    lat: f64,
    frequency: f64,
) -> Noise {
    let mut n = Noise::default();
    if let ManMade::Custom(v) = kind
        && v < 0.0
    {
        return Noise {
            fa_a: 0.0,
            du_a: 0.0,
            dl_a: 0.0,
            fa_m: v,
            du_m: 0.0,
            dl_m: 0.0,
            fa_g: 0.0,
            du_g: 0.0,
            dl_g: 0.0,
            du_t: 0.0,
            dl_t: 0.0,
            fam_t: -v,
        };
    }
    atmospheric(c, &mut n, hour, lng, lat, frequency);
    n.fa_g = 52.0 - 23.0 * frequency.log10();
    n.du_g = 2.0;
    n.dl_g = 2.0;
    man_made(&mut n, kind, frequency);
    let fa = [n.fa_a, n.fa_g, n.fa_m];
    let (upper, du_t) = total(fa, [n.du_a, n.du_g, n.du_m]);
    let (lower, dl_t) = total(fa, [n.dl_a, n.dl_g, n.dl_m]);
    n.du_t = du_t;
    n.dl_t = dl_t;
    n.fam_t = upper.min(lower);
    n
}
