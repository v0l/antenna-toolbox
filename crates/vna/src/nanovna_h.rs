use crate::{Error, Point, Result, Vna, shell};
use serialport::SerialPort;
use std::io::{Read, Write};
use web_time::{Duration, Instant};

pub struct NanoVnaH {
    port: Box<dyn SerialPort>,
    info: String,
    version: String,
    max_points: usize,
}

impl NanoVnaH {
    pub fn open(path: &str) -> Result<Self> {
        let port = serialport::new(path, 115_200).timeout(Duration::from_millis(200)).open()?;
        let mut vna =
            NanoVnaH { port, info: String::new(), version: String::new(), max_points: 101 };
        vna.drain();
        vna.version = vna.command("version", Duration::from_secs(3))?.join(" ");
        let info = vna.command("info", Duration::from_secs(3))?;
        vna.max_points = shell::max_points(&info);
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
        while !out.ends_with(shell::PROMPT) {
            if Instant::now() > deadline {
                return Err(Error::Protocol(format!("timed out waiting for `{cmd}`")));
            }
            match self.port.read(&mut buf) {
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => return Err(e.into()),
            }
        }
        shell::reply(&out, cmd)
            .unwrap_or_else(|| Err(Error::Protocol(format!("no prompt after `{cmd}`"))))
    }

    pub fn cal_status(&mut self) -> Result<String> {
        Ok(self.command("cal", Duration::from_secs(2))?.join(" "))
    }
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

    fn device_cal_status(&mut self) -> Option<String> {
        self.cal_status().ok()
    }

    fn scan(
        &mut self,
        start_hz: f64,
        stop_hz: f64,
        points: usize,
        s21: bool,
    ) -> Result<Vec<Point>> {
        let cmd = shell::scan_command(start_hz, stop_hz, points, s21);
        let lines = self.command(&cmd, Duration::from_secs(60))?;
        shell::parse_scan(&lines, s21, points)
    }
}
