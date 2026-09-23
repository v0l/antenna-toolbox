mod cal;
mod nanovna_h;
mod nanovna_v2;

pub use cal::{Calibration, Standard};
pub use nanovna_h::NanoVnaH;
pub use nanovna_v2::NanoVnaV2;
pub use num_complex::Complex64 as C64;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Serial(serialport::Error),
    Io(std::io::Error),
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Serial(e) => write!(f, "serial: {e}"),
            Error::Io(e) => write!(f, "io: {e}"),
            Error::Protocol(e) => write!(f, "protocol: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Self {
        Error::Serial(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub freq: f64,
    pub s11: C64,
    pub s21: Option<C64>,
}

impl Point {
    pub fn z(&self, z0: f64) -> C64 {
        let one = C64::new(1.0, 0.0);
        z0 * (one + self.s11) / (one - self.s11)
    }

    pub fn swr(&self) -> f64 {
        let g = self.s11.norm();
        if g >= 0.9999 { 99.0 } else { ((1.0 + g) / (1.0 - g)).min(99.0) }
    }

    pub fn return_loss_db(&self) -> f64 {
        -20.0 * self.s11.norm().max(1e-9).log10()
    }
}

pub trait Vna: Send {
    fn describe(&self) -> String;
    fn max_points(&self) -> usize;
    fn range_hz(&self) -> (f64, f64);
    fn calibrated_on_device(&self) -> bool;
    fn scan(&mut self, start_hz: f64, stop_hz: f64, points: usize, s21: bool) -> Result<Vec<Point>>;

    fn sweep(&mut self, start_hz: f64, stop_hz: f64, points: usize, s21: bool) -> Result<Vec<Point>> {
        let points = points.max(2);
        let chunk = self.max_points().max(2);
        if points <= chunk {
            return self.scan(start_hz, stop_hz, points, s21);
        }
        let step = (stop_hz - start_hz) / (points - 1) as f64;
        let mut out = Vec::with_capacity(points);
        let mut i = 0;
        while i < points {
            let n = chunk.min(points - i);
            let n = if n == 1 { 2 } else { n };
            let a = start_hz + step * i as f64;
            let b = a + step * (n - 1) as f64;
            let part = self.scan(a, b, n, s21)?;
            out.extend(part.into_iter().take(points - i));
            i += n;
        }
        Ok(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    NanoVnaH,
    NanoVnaV2,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::NanoVnaH => "NanoVNA-H / H4 (text shell)",
            Kind::NanoVnaV2 => "NanoVNA V2 / LiteVNA (binary)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Port {
    pub path: String,
    pub kind: Kind,
    pub product: String,
}

pub fn detect() -> Vec<Port> {
    let Ok(ports) = serialport::available_ports() else {
        return Vec::new();
    };
    ports
        .into_iter()
        .filter_map(|p| match p.port_type {
            serialport::SerialPortType::UsbPort(u) => {
                let kind = match (u.vid, u.pid) {
                    (0x0483, 0x5740) => Kind::NanoVnaH,
                    (0x04b4, 0x0008) => Kind::NanoVnaV2,
                    _ => return None,
                };
                Some(Port {
                    path: p.port_name,
                    kind,
                    product: u.product.unwrap_or_else(|| kind.label().to_string()),
                })
            }
            _ => None,
        })
        .collect()
}

pub fn open(port: &Port) -> Result<Box<dyn Vna>> {
    Ok(match port.kind {
        Kind::NanoVnaH => Box::new(NanoVnaH::open(&port.path)?),
        Kind::NanoVnaV2 => Box::new(NanoVnaV2::open(&port.path)?),
    })
}
