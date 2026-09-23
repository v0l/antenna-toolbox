use crate::vec::Vec3;
use std::sync::Arc;

pub type Tri = [usize; 3];

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<Tri>,
}

#[derive(Clone, Debug)]
pub struct SolveLine {
    pub pts: Vec<Vec3>,
    pub rad: Option<f64>,
}

impl From<Vec<Vec3>> for SolveLine {
    fn from(pts: Vec<Vec3>) -> Self {
        Self { pts, rad: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePlane {
    pub axis: usize,
    pub at: f64,
}

pub type Blocked = Arc<dyn Fn(Vec3) -> bool + Send + Sync>;

#[derive(Clone, Default)]
pub struct WireGeometry {
    pub lines: Vec<SolveLine>,
    pub feed: Vec3,
    pub ground_z: Option<f64>,
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
