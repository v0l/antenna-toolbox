use crate::{C64, Error, Point, Result, Vna};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const PROMPT: &[u8] = b"ch> ";

pub struct NanoVnaH {
    port: Box<dyn SerialPort>,
    info: String,
    version: String,
    max_points: usize,
}

impl NanoVnaH {
    pub fn open(path: &str) -> Result<Self> {
        let port = serialport::new(path, 115_200).timeout(Duration::from_millis(200)).open()?;
        let mut vna = NanoVnaH { port, info: String::new(), version: String::new(), max_points: 101 };
        vna.drain();
        vna.version = vna.command("version", Duration::from_secs(3))?.join(" ");
        let info = vna.command("info", Duration::from_secs(3))?;
        vna.max_points = info
            .iter()
            .find_map(|l| {
                let at = l.find("p:")?;
                l[at + 2..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
            })
            .unwrap_or(101);
        vna.info = info.first().cloned().unwrap_or_default();
        Ok(vna)
    }

    fn drain(&mut self) {
        let _ = self.port.write_all(b"\r");
        let mut buf = [0u8; 4096];
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            match self.port.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    pub fn command(&mut self, cmd: &str, timeout: Duration) -> Result<Vec<String>> {
        self.port.write_all(cmd.as_bytes())?;
        self.port.write_all(b"\r")?;
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        let deadline = Instant::now() + timeout;
        while !out.ends_with(PROMPT) {
            if Instant::now() > deadline {
                return Err(Error::Protocol(format!("timed out waiting for `{cmd}`")));
            }
            match self.port.read(&mut buf) {
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => return Err(e.into()),
            }
        }
        let text = String::from_utf8_lossy(&out[..out.len() - PROMPT.len()]).into_owned();
        let mut lines = text.split("\r\n");
        let echo = lines.next().unwrap_or_default();
        if echo.trim() != cmd {
            return Err(Error::Protocol(format!("unexpected echo `{echo}` for `{cmd}`")));
        }
        Ok(lines.filter(|l| !l.is_empty()).map(str::to_string).collect())
    }

    pub fn cal_status(&mut self) -> Result<String> {
        Ok(self.command("cal", Duration::from_secs(2))?.join(" "))
    }
}

fn parse_scan(lines: &[String], s21: bool) -> Result<Vec<Point>> {
    lines
        .iter()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            let need = if s21 { 5 } else { 3 };
            if v.len() < need {
                return Err(Error::Protocol(format!("short scan line `{l}`")));
            }
            Ok(Point {
                freq: v[0],
                s11: C64::new(v[1], v[2]),
                s21: s21.then(|| C64::new(v[3], v[4])),
            })
        })
        .collect()
}

impl Vna for NanoVnaH {
    fn describe(&self) -> String {
        format!("{} · firmware {} · {} points", self.info, self.version, self.max_points)
    }

    fn max_points(&self) -> usize {
        self.max_points
    }

    fn range_hz(&self) -> (f64, f64) {
        (10e3, 1.5e9)
    }

    fn calibrated_on_device(&self) -> bool {
        true
    }

    fn scan(&mut self, start_hz: f64, stop_hz: f64, points: usize, s21: bool) -> Result<Vec<Point>> {
        let mask = if s21 { 7 } else { 3 };
        let cmd = format!("scan {} {} {} {mask}", start_hz.round() as u64, stop_hz.round() as u64, points);
        let lines = self.command(&cmd, Duration::from_secs(60))?;
        let pts = parse_scan(&lines, s21)?;
        if pts.len() != points {
            return Err(Error::Protocol(format!("asked for {points} points, got {}", pts.len())));
        }
        Ok(pts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scan_output() {
        let lines = vec!["868000000 0.123 -0.456".to_string(), "869000000 0.1 0.2".to_string()];
        let p = parse_scan(&lines, false).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].freq, 868e6);
        assert_eq!(p[0].s11, C64::new(0.123, -0.456));
        assert!(p[0].s21.is_none());
    }
}
