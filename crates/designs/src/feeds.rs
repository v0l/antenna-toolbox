use crate::draw::GOLD;
use crate::{DIPOLE_K, Wire, wire};
use antenna_solver::vec::Vec3;
use std::f64::consts::{PI, SQRT_2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedKind {
    Dipole,
    Biquad,
    Helix,
}

pub const FEED_KINDS: [FeedKind; 3] = [FeedKind::Dipole, FeedKind::Biquad, FeedKind::Helix];

impl FeedKind {
    pub fn from_control(v: f64) -> Self {
        FEED_KINDS.get(v as usize).copied().unwrap_or(FeedKind::Dipole)
    }

    pub fn label(self) -> &'static str {
        match self {
            FeedKind::Dipole => "Half-wave dipole",
            FeedKind::Biquad => "Biquad",
            FeedKind::Helix => "Axial helix (circular)",
        }
    }
}

pub struct FeedFrame {
    pub origin: Vec3,
    pub bore: Vec3,
    pub pol: Vec3,
}

pub struct FeedGeom {
    pub lines: Vec<Vec<Vec3>>,
    pub feed: Vec3,
    pub wires: Vec<Wire>,
    pub pol: &'static str,
    pub note: &'static str,
    pub depth: f64,
    pub z_unreliable: bool,
}

pub fn build_feed(kind: FeedKind, lam: f64, frame: &FeedFrame, driven: Option<f64>) -> FeedGeom {
    let driven = driven.unwrap_or(DIPOLE_K * lam);
    let FeedFrame { origin, bore, pol } = *frame;
    let v = [
        bore[1] * pol[2] - bore[2] * pol[1],
        bore[2] * pol[0] - bore[0] * pol[2],
        bore[0] * pol[1] - bore[1] * pol[0],
    ];
    let at = |u: f64, vv: f64, w: f64| -> Vec3 {
        [
            origin[0] + u * pol[0] + vv * v[0] + w * bore[0],
            origin[1] + u * pol[1] + vv * v[1] + w * bore[1],
            origin[2] + u * pol[2] + vv * v[2] + w * bore[2],
        ]
    };

    match kind {
        FeedKind::Dipole => {
            let half = driven / 2.0;
            let stub = half * 0.04;
            FeedGeom {
                lines: vec![vec![at(-half, 0.0, 0.0), at(half, 0.0, 0.0)]],
                feed: at(0.0, 0.0, 0.0),
                wires: vec![
                    wire(vec![at(-half, 0.0, 0.0), at(-stub, 0.0, 0.0)], GOLD, 3.5),
                    wire(vec![at(stub, 0.0, 0.0), at(half, 0.0, 0.0)], GOLD, 3.5),
                ],
                pol: "polarisation: linear, along the feed dipole",
                note: "A half-wave dipole is the plain choice: broad enough to light the whole \
                       reflector, and it spills past the edge, which is where most of the loss \
                       goes.",
                depth: 0.0,
                z_unreliable: false,
            }
        }
        FeedKind::Biquad => {
            let side = 0.25 * lam;
            let hd = side * SQRT_2 / 2.0;
            let e = lam / 120.0;
            let g = lam / 120.0;
            FeedGeom {
                lines: vec![
                    vec![
                        at(-e, g, 0.0),
                        at(-hd, hd, 0.0),
                        at(0.0, 2.0 * hd, 0.0),
                        at(hd, hd, 0.0),
                        at(e, g, 0.0),
                    ],
                    vec![
                        at(e, -g, 0.0),
                        at(hd, -hd, 0.0),
                        at(0.0, -2.0 * hd, 0.0),
                        at(-hd, -hd, 0.0),
                        at(-e, -g, 0.0),
                    ],
                    vec![at(e, g, 0.0), at(e, -g, 0.0)],
                    vec![at(-e, g, 0.0), at(-e, -g, 0.0)],
                ],
                feed: at(-e, 0.0, 0.0),
                wires: vec![
                    wire(
                        vec![
                            at(0.0, hd * 0.07, 0.0),
                            at(-hd, hd, 0.0),
                            at(0.0, 2.0 * hd, 0.0),
                            at(hd, hd, 0.0),
                            at(0.0, hd * 0.07, 0.0),
                        ],
                        GOLD,
                        3.5,
                    ),
                    wire(
                        vec![
                            at(0.0, -hd * 0.07, 0.0),
                            at(-hd, -hd, 0.0),
                            at(0.0, -2.0 * hd, 0.0),
                            at(-hd, -hd, 0.0),
                            at(0.0, -hd * 0.07, 0.0),
                        ],
                        GOLD,
                        3.5,
                    ),
                ],
                pol: "polarisation: linear, across the diamonds",
                note: "A biquad feed lights the reflector with a broad linear pattern. It is \
                       fussier than a dipole about its gap, so check the solved pattern before \
                       trusting the gain.",
                depth: 0.0,
                z_unreliable: false,
            }
        }
        FeedKind::Helix => {
            let turns = 3.0;
            let r_h = lam / (2.0 * PI);
            let s = 0.22 * lam;
            let back = turns * s / 2.0;
            let pts: Vec<Vec3> = (0..=(turns as usize * 36))
                .map(|i| {
                    let t = i as f64 / 36.0;
                    at(r_h * (t * 2.0 * PI).cos(), r_h * (t * 2.0 * PI).sin(), t * s - back)
                })
                .collect();
            let stub_end = at(r_h, 0.0, -back - lam / 60.0);
            FeedGeom {
                lines: vec![pts.clone(), vec![stub_end, pts[0]]],
                feed: at(r_h, 0.0, -back - lam / 120.0),
                wires: vec![wire(pts.clone(), GOLD, 2.6), wire(vec![stub_end, pts[0]], GOLD, 3.5)],
                pol: "polarisation: circular, set by the winding sense of the helix",
                note: "A helix gives circular polarisation, so the reflector returns the opposite \
                       sense and a matching receiver has to be wound the same way round as this \
                       one. Three turns is about right for a reflector: more turns narrows the \
                       beam until it stops filling the dish. It is modelled without the ground cup \
                       a real one needs at its base, so take the pattern and ignore the \
                       impedance: a built axial helix lands near 140 ohms.",
                depth: turns * s,
                z_unreliable: true,
            }
        }
    }
}
