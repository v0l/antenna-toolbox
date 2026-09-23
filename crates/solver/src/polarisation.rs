use crate::mom::FieldFn;
use crate::vec::{Vec3, cross, dot, normalise};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Rhcp,
    Lhcp,
}

impl Hand {
    pub fn label(self) -> &'static str {
        match self {
            Hand::Rhcp => "RHCP",
            Hand::Lhcp => "LHCP",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    pub ar_db: f64,
    pub hand: Hand,
    pub circularity: f64,
}

pub fn ellipse_at(field: &FieldFn, dir: Vec3) -> Ellipse {
    let r = normalise(dir);
    let seed = if r[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let a = normalise(cross(seed, r));
    let b = cross(r, a);

    let (er, ei) = field(r);
    let (ar, ai, br, bi) = (dot(er, a), dot(ei, a), dot(er, b), dot(ei, b));

    let r_r = (ar - bi).hypot(ai + br) / std::f64::consts::SQRT_2;
    let r_l = (ar + bi).hypot(ai - br) / std::f64::consts::SQRT_2;

    let big = r_r.max(r_l);
    let small = r_r.min(r_l);
    let denom = big - small;
    let ar_db =
        if denom < 1e-12 * big { 60.0 } else { (20.0 * ((big + small) / denom).log10()).min(60.0) };
    let total = r_r * r_r + r_l * r_l;
    Ellipse {
        ar_db,
        hand: if r_r >= r_l { Hand::Rhcp } else { Hand::Lhcp },
        circularity: if total > 0.0 { big * big / total } else { 0.0 },
    }
}
