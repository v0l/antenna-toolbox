pub mod designs;
pub mod draw;
pub mod export;
pub mod feed_detail;
pub mod feeds;
pub mod matching;

use antenna_solver::geometry::{Geometry, Mesh};
use antenna_solver::vec::Vec3;
use draw::{Drawing, Rgba};
use feed_detail::FeedDetail;
use std::cell::RefCell;
use std::collections::HashMap;

pub use designs::{DESIGNS, by_id};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlId {
    Spacing,
    Turns,
    Droop,
    Spokes,
    Angle,
    Segments,
    FeedType,
    Cant,
    Hand,
    Sections,
    Phasing,
    CoilTurns,
    LoopCirc,
    FOverD,
    Flare,
    SpiralTurns,
    FeedGap,
    Mouth,
}

pub struct ControlDef {
    pub label: &'static str,
    pub def: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub integer: bool,
    pub options: &'static [&'static str],
}

const fn ctl(
    label: &'static str,
    def: f64,
    min: f64,
    max: f64,
    step: f64,
    integer: bool,
) -> ControlDef {
    ControlDef { label, def, min, max, step, integer, options: &[] }
}

impl ControlId {
    pub const ALL: [ControlId; 18] = [
        ControlId::Spacing,
        ControlId::Turns,
        ControlId::Droop,
        ControlId::Spokes,
        ControlId::Angle,
        ControlId::Segments,
        ControlId::FeedType,
        ControlId::Cant,
        ControlId::Hand,
        ControlId::Sections,
        ControlId::Phasing,
        ControlId::CoilTurns,
        ControlId::LoopCirc,
        ControlId::FOverD,
        ControlId::Flare,
        ControlId::SpiralTurns,
        ControlId::FeedGap,
        ControlId::Mouth,
    ];

    pub fn def(self) -> ControlDef {
        use ControlId::*;
        match self {
            Spacing => ctl("Reflector spacing (λ)", 0.18, 0.08, 0.3, 0.01, false),
            Turns => ctl("Number of turns", 8.0, 3.0, 30.0, 1.0, true),
            Droop => ctl("Radial droop (degrees below horizontal)", 45.0, 0.0, 60.0, 5.0, false),
            Spokes => ctl("Spokes per section", 8.0, 4.0, 16.0, 1.0, true),
            Angle => ctl("Included angle between arms", 120.0, 60.0, 180.0, 5.0, false),
            Segments => ctl("Dish segments (gores)", 12.0, 4.0, 32.0, 1.0, true),
            FeedType => ControlDef {
                options: &["Half-wave dipole", "Biquad", "Axial helix (circular)"],
                ..ctl("Feed type", 0.0, 0.0, 2.0, 1.0, true)
            },
            Cant => ctl("Cant from horizontal", 30.0, 10.0, 60.0, 5.0, false),
            Hand => ControlDef {
                options: &["Right hand (RHCP)", "Left hand (LHCP)"],
                ..ctl("Polarisation sense", 0.0, 0.0, 1.0, 1.0, true)
            },
            Sections => ctl("Stacked sections", 4.0, 2.0, 8.0, 1.0, true),
            Phasing => ControlDef {
                options: &["Fitted, beam on the horizon", "Removed, watch it fail"],
                ..ctl("Phasing hairpins", 0.0, 0.0, 1.0, 1.0, true)
            },
            CoilTurns => ctl("Turns on the coil", 6.0, 2.0, 20.0, 1.0, true),
            LoopCirc => ctl("Loop circumference (λ)", 1.0, 0.1, 1.5, 0.05, false),
            FOverD => ctl("Focal length / diameter", 0.4, 0.25, 0.7, 0.05, false),
            Flare => ctl("Flare angle (degrees)", 90.0, 20.0, 150.0, 5.0, false),
            SpiralTurns => ctl("Spiral turns per arm", 2.0, 1.5, 4.0, 0.5, false),
            FeedGap => ctl("Feed gap (% of disc radius)", 6.0, 1.0, 20.0, 1.0, false),
            Mouth => ctl("Mouth opening (λ)", 0.5, 0.25, 1.2, 0.05, false),
        }
    }

    pub fn clamp(self, value: f64) -> f64 {
        let d = self.def();
        if !value.is_finite() {
            return d.def;
        }
        let v = value.clamp(d.min, d.max);
        if d.integer { v.round() } else { v }
    }
}

pub type Controls = HashMap<ControlId, f64>;

pub fn default_controls() -> Controls {
    ControlId::ALL.iter().map(|&c| (c, c.def().def)).collect()
}

pub const FREQUENCY_PRESETS: [f64; 8] = [144.2, 162.0, 433.9, 868.0, 915.0, 1090.0, 2450.0, 5800.0];

pub const WIRE_PRESETS: [(&str, f64); 5] =
    [("12ga", 2.05), ("14ga", 1.63), ("16ga", 1.29), ("18ga", 1.02), ("22ga", 0.64)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Beam,
    TwoSide,
    Omni,
}

impl Group {
    pub const ALL: [(Group, &'static str, &'static str); 3] = [
        (Group::Beam, "one way", "a single forward lobe, point it at the other end"),
        (Group::TwoSide, "two ways", "equal lobes front and back, nulls off the edges"),
        (Group::Omni, "all round", "the same in every direction you turn"),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    Wire,
    Sheet,
    Both,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub total: bool,
}

pub fn row(label: impl Into<String>, value: impl Into<String>) -> Row {
    Row { label: label.into(), value: value.into(), total: false }
}

pub fn total(label: impl Into<String>, value: impl Into<String>) -> Row {
    Row { label: label.into(), value: value.into(), total: true }
}

#[derive(Clone, Debug)]
pub struct Wire {
    pub p: Vec<Vec3>,
    pub c: Rgba,
    pub w: f32,
    pub thin: bool,
}

pub fn wire(p: Vec<Vec3>, c: Rgba, w: f32) -> Wire {
    Wire { p, c, w, thin: false }
}

#[derive(Clone, Debug)]
pub struct Poly {
    pub p: Vec<Vec3>,
    pub fill: Option<Rgba>,
    pub stroke: Option<Rgba>,
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub wires: Vec<Wire>,
    pub polys: Vec<Poly>,
    pub mesh: Option<Mesh>,
    pub feed: Option<Vec3>,
    pub beam_from: Option<f64>,
    pub beam_vec: Option<Vec3>,
    pub omni: bool,
    pub omni_y: Option<f64>,
    pub pol: String,
    pub pattern_origin: Option<Vec3>,
    pub up: Option<Vec3>,
}

impl Scene {
    pub fn up(&self) -> Vec3 {
        self.up.unwrap_or([0.0, 1.0, 0.0])
    }

    pub fn beam_direction(&self) -> Vec3 {
        self.beam_vec.unwrap_or([0.0, 0.0, 1.0])
    }
}

#[derive(Clone, Debug)]
pub struct CutFile {
    pub loops: Vec<Vec<Vec3>>,
    pub circles: Vec<([f64; 2], f64)>,
    pub note: String,
}

pub struct Output {
    pub spec: String,
    pub rows: Vec<Row>,
    pub scene: Scene,
    pub solve: Geometry,
    pub diagram: Drawing,
    pub feed: FeedDetail,
    pub notes: String,
    pub cut: Option<CutFile>,
}

#[derive(Clone, Debug)]
pub struct Tunable {
    pub key: String,
    pub name: String,
    pub val: f64,
    pub def: f64,
}

pub struct Ctx<'a> {
    pub lam: f64,
    pub wire_dia: f64,
    pub fmt: &'a dyn Fn(f64) -> String,
    pub design: &'static str,
    pub overrides: &'a HashMap<String, f64>,
    pub controls: &'a Controls,
    pub scale: f64,
    pub params: RefCell<Vec<Tunable>>,
}

impl Ctx<'_> {
    #[allow(non_snake_case)]
    pub fn P(&self, name: &str, value: f64) -> f64 {
        let key = format!("{}:{}", self.design, name);
        let val = self.overrides.get(&key).copied().filter(|v| v.is_finite()).unwrap_or(value)
            * self.scale;
        self.params.borrow_mut().push(Tunable { key, name: name.to_string(), val, def: value });
        val
    }

    pub fn ctl(&self, id: ControlId) -> f64 {
        id.clamp(self.controls.get(&id).copied().unwrap_or(id.def().def))
    }

    pub fn fmt(&self, mm: f64) -> String {
        (self.fmt)(mm)
    }
}

pub struct Design {
    pub id: &'static str,
    pub name: &'static str,
    pub group: Group,
    pub build: Build,
    pub gain: &'static str,
    pub controls: &'static [ControlId],
    pub polarisation: &'static str,
    pub compute: fn(&Ctx) -> Output,
}

pub struct Computed {
    pub output: Output,
    pub params: Vec<Tunable>,
}

impl Design {
    pub fn run(
        &'static self,
        lam: f64,
        wire_dia: f64,
        fmt: &dyn Fn(f64) -> String,
        overrides: &HashMap<String, f64>,
        controls: &Controls,
        scale: f64,
    ) -> Computed {
        let ctx = Ctx {
            lam,
            wire_dia,
            fmt,
            design: self.id,
            overrides,
            controls,
            scale,
            params: RefCell::new(Vec::new()),
        };
        let output = (self.compute)(&ctx);
        Computed { output, params: ctx.params.into_inner() }
    }
}

pub const DIPOLE_K: f64 = 0.478;
