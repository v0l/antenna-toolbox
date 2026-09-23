use crate::{C64, Error, Point, Result, Vna};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::thread::sleep;
use std::time::Duration;

const READ: u8 = 0x10;
const READFIFO: u8 = 0x18;
const WRITE: u8 = 0x20;
const WRITE2: u8 = 0x21;
const WRITE8: u8 = 0x23;

const SWEEP_START: u8 = 0x00;
const SWEEP_STEP: u8 = 0x10;
const SWEEP_POINTS: u8 = 0x20;
const VALUES_PER_FREQ: u8 = 0x22;
const VALUES_FIFO: u8 = 0x30;
const DEVICE_VARIANT: u8 = 0xf0;
const HARDWARE_REVISION: u8 = 0xf2;
const FW_MAJOR: u8 = 0xf3;
const FW_MINOR: u8 = 0xf4;

const RECORD: usize = 32;
const SETTLE: Duration = Duration::from_millis(50);

pub struct NanoVnaV2 {
    port: Box<dyn SerialPort>,
    variant: u8,
    hardware: u8,
    firmware: (u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub fwd: C64,
    pub rev0: C64,
    pub rev1: C64,
    pub index: u16,
}

pub fn parse_record(b: &[u8]) -> Record {
    let i = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64;
    Record {
        fwd: C64::new(i(0), i(4)),
        rev0: C64::new(i(8), i(12)),
        rev1: C64::new(i(16), i(20)),
        index: u16::from_le_bytes([b[24], b[25]]),
    }
}

impl NanoVnaV2 {
    pub fn open(path: &str) -> Result<Self> {
        let port = serialport::new(path, 115_200).timeout(Duration::from_millis(1000)).open()?;
        let mut vna = NanoVnaV2 { port, variant: 0, hardware: 0, firmware: (0, 0) };
        vna.reset()?;
        let [major, minor] = vna.read2(FW_MAJOR, FW_MINOR)?;
        if major == 0xff {
            return Err(Error::Protocol("device is in DFU mode".into()));
        }
        let [variant, hardware] = vna.read2(DEVICE_VARIANT, HARDWARE_REVISION)?;
        vna.firmware = (major, minor);
        vna.variant = variant;
        vna.hardware = hardware;
        Ok(vna)
    }

    fn reset(&mut self) -> Result<()> {
        self.port.write_all(&[0u8; 8])?;
        sleep(SETTLE);
        let _ = self.port.clear(serialport::ClearBuffer::Input);
        Ok(())
    }

    fn read2(&mut self, a: u8, b: u8) -> Result<[u8; 2]> {
        self.port.write_all(&[READ, a, READ, b])?;
        let mut out = [0u8; 2];
        self.port.read_exact(&mut out)?;
        Ok(out)
    }

    fn configure(&mut self, start: u64, step: u64, points: u16) -> Result<()> {
        let mut cmd = vec![WRITE8, SWEEP_START];
        cmd.extend(start.to_le_bytes());
        cmd.extend([WRITE8, SWEEP_STEP]);
        cmd.extend(step.to_le_bytes());
        cmd.extend([WRITE2, SWEEP_POINTS]);
        cmd.extend(points.to_le_bytes());
        cmd.extend([WRITE2, VALUES_PER_FREQ]);
        cmd.extend(1u16.to_le_bytes());
        self.port.write_all(&cmd)?;
        sleep(SETTLE);
        Ok(())
    }
}

impl Vna for NanoVnaV2 {
    fn describe(&self) -> String {
        format!(
            "NanoVNA V2 family · variant {} · hardware {} · firmware {}.{}",
            self.variant, self.hardware, self.firmware.0, self.firmware.1
        )
    }

    fn max_points(&self) -> usize {
        1024
    }

    fn range_hz(&self) -> (f64, f64) {
        (50e3, 4.4e9)
    }

    fn calibrated_on_device(&self) -> bool {
        false
    }

    fn scan(
        &mut self,
        start_hz: f64,
        stop_hz: f64,
        points: usize,
        _s21: bool,
    ) -> Result<Vec<Point>> {
        let step = (stop_hz - start_hz) / (points - 1) as f64;
        self.configure(start_hz.round() as u64, step.round() as u64, points as u16)?;
        self.reset()?;
        self.port.write_all(&[WRITE, VALUES_FIFO, 0])?;
        sleep(SETTLE);
        let _ = self.port.clear(serialport::ClearBuffer::Input);

        let mut got: Vec<Option<Record>> = vec![None; points];
        let mut left = points;
        let mut buf = vec![0u8; 255 * RECORD];
        while left > 0 {
            let n = left.min(255);
            self.port.write_all(&[READFIFO, VALUES_FIFO, n as u8])?;
            self.port.read_exact(&mut buf[..n * RECORD])?;
            for rec in buf[..n * RECORD].chunks_exact(RECORD).map(parse_record) {
                if let Some(slot) = got.get_mut(rec.index as usize) {
                    *slot = Some(rec);
                }
            }
            left -= n;
        }
        got.into_iter()
            .enumerate()
            .map(|(i, r)| {
                let r = r.ok_or_else(|| Error::Protocol(format!("no data for point {i}")))?;
                Ok(Point {
                    freq: start_hz + step.round() * i as f64,
                    s11: r.rev0 / r.fwd,
                    s21: Some(r.rev1 / r.fwd),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_fifo_record() {
        let mut b = [0u8; 32];
        for (i, v) in [1000i32, -2000, 300, 400, -50, 60].iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b[24..26].copy_from_slice(&7u16.to_le_bytes());
        let r = parse_record(&b);
        assert_eq!(r.fwd, C64::new(1000.0, -2000.0));
        assert_eq!(r.rev0, C64::new(300.0, 400.0));
        assert_eq!(r.rev1, C64::new(-50.0, 60.0));
        assert_eq!(r.index, 7);
    }
}
