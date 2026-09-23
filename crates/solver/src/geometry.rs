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

#[derive(Clone, Default)]
pub struct WireGeometry {
    pub lines: Vec<SolveLine>,
    pub feed: Vec3,
    pub ground_z: Option<f64>,
    pub real_ground: Option<RealGround>,
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
}

impl Geometry {
    pub fn is_surface(&self) -> bool {
        matches!(self, Geometry::Surface(_))
    }
}
