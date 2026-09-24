use crate::vec::Vec3;
use std::sync::Arc;

pub type Tri = [usize; 3];

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<Tri>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Insulation {
    pub eps_r: f64,
    pub tan_d: f64,
    pub inner: f64,
    pub outer: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WireProps {
    pub conductivity: Option<f64>,
    pub insulation: Option<Insulation>,
}

pub const COPPER: f64 = 5.8e7;
pub const ALUMINIUM: f64 = 3.5e7;
pub const BRASS: f64 = 1.5e7;
pub const STEEL: f64 = 1.4e6;

#[derive(Clone, Debug)]
pub struct SolveLine {
    pub pts: Vec<Vec3>,
    pub rad: Option<f64>,
    pub props: WireProps,
    pub segments: Option<usize>,
}

impl From<Vec<Vec3>> for SolveLine {
    fn from(pts: Vec<Vec3>) -> Self {
        Self { pts, rad: None, props: WireProps::default(), segments: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePlane {
    pub axis: usize,
    pub at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RealGround {
    pub eps_r: f64,
    pub sigma: f64,
}

impl RealGround {
    pub const AVERAGE: RealGround = RealGround { eps_r: 13.0, sigma: 0.005 };
    pub const POOR: RealGround = RealGround { eps_r: 5.0, sigma: 0.001 };
    pub const GOOD: RealGround = RealGround { eps_r: 20.0, sigma: 0.03 };
    pub const SEA: RealGround = RealGround { eps_r: 80.0, sigma: 5.0 };
    pub const FRESH_WATER: RealGround = RealGround { eps_r: 80.0, sigma: 0.001 };
}

pub type Blocked = Arc<dyn Fn(Vec3) -> bool + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Load {
    Series { r: f64, l: f64, c: f64 },
    Parallel { r: f64, l: f64, c: f64 },
    Impedance { r: f64, x: f64 },
}

impl Load {
    pub fn impedance(self, freq_hz: f64) -> crate::C64 {
        use crate::C64;
        let w = 2.0 * std::f64::consts::PI * freq_hz;
        match self {
            Load::Impedance { r, x } => C64::new(r, x),
            Load::Series { r, l, c } => {
                let xc = if c > 0.0 { -1.0 / (w * c) } else { 0.0 };
                C64::new(r, w * l + xc)
            }
            Load::Parallel { r, l, c } => {
                let mut y = C64::new(0.0, 0.0);
                if r > 0.0 {
                    y += 1.0 / r;
                }
                if l > 0.0 {
                    y += C64::new(0.0, -1.0 / (w * l));
                }
                if c > 0.0 {
                    y += C64::new(0.0, w * c);
                }
                if y.norm() > 0.0 { y.inv() } else { C64::new(1e12, 0.0) }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Network {
    Line { z0: f64, length: f64, crossed: bool, shunt: [crate::C64; 2] },
    Admittance { y11: crate::C64, y12: crate::C64, y22: crate::C64 },
}

impl Network {
    pub fn y(self, freq_hz: f64) -> [[crate::C64; 2]; 2] {
        use crate::C64;
        match self {
            Network::Line { z0, length, crossed, shunt } => {
                let bl = 2.0 * std::f64::consts::PI * freq_hz / 299_792_458.0 * length / 1000.0;
                let s = if bl.sin().abs() < 1e-9 { 1e-9_f64.copysign(bl.sin()) } else { bl.sin() };
                let y11 = C64::new(0.0, -bl.cos() / s / z0);
                let y12 = C64::new(0.0, 1.0 / s / z0) * if crossed { -1.0 } else { 1.0 };
                [[y11 + shunt[0], y12], [y12, y11 + shunt[1]]]
            }
            Network::Admittance { y11, y12, y22 } => [[y11, y12], [y12, y22]],
        }
    }
}

#[derive(Clone, Default)]
pub struct WireGeometry {
    pub lines: Vec<SolveLine>,
    pub feed: Vec3,
    pub sources: Vec<(Vec3, crate::C64)>,
    pub loads: Vec<(Vec3, Load)>,
    pub networks: Vec<(Vec3, Vec3, Network)>,
    pub ground_z: Option<f64>,
    pub real_ground: Option<RealGround>,
    pub sommerfeld: bool,
    pub mirrors: Vec<ImagePlane>,
    pub blocked: Option<Blocked>,
    pub po: Option<Mesh>,
}

impl WireGeometry {
    pub fn new(lines: Vec<Vec<Vec3>>, feed: Vec3) -> Self {
        Self { lines: lines.into_iter().map(SolveLine::from).collect(), feed, ..Default::default() }
    }

    pub fn ground(mut self, z: f64) -> Self {
        self.ground_z = Some(z);
        self
    }

    pub fn with_props(mut self, props: WireProps) -> Self {
        for l in &mut self.lines {
            l.props = props;
        }
        self
    }
}

#[derive(Clone, Debug)]
pub struct SurfaceGeometry {
    pub mesh: Mesh,
    pub feed: Vec3,
    pub feed_dir: Vec3,
    pub feed_tol: f64,
}

#[derive(Clone)]
pub enum Geometry {
    Wire(WireGeometry),
    Surface(SurfaceGeometry),
    Volume(crate::fdtd::Model),
}

impl Geometry {
    pub fn is_surface(&self) -> bool {
        matches!(self, Geometry::Surface(_))
    }
}
