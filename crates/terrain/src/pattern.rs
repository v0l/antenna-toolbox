use rayon::prelude::*;

const EL: usize = 181;
const AZ: usize = 360;

#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub freq_mhz: Option<f64>,
    pub absolute: bool,
    pub mirror_below: bool,
    db: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mount {
    pub heading: f64,
    pub tilt: f64,
    pub roll: f64,
}

type V3 = [f64; 3];
type Cut = Vec<(f64, f64)>;

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn enu(bearing: f64, el: f64) -> V3 {
    let (b, e) = (bearing.to_radians(), el.to_radians());
    [e.cos() * b.sin(), e.cos() * b.cos(), e.sin()]
}

impl Mount {
    pub fn to_antenna(&self, bearing: f64, el: f64) -> (f64, f64) {
        let (h, t) = (self.heading.to_radians(), self.tilt.to_radians());
        let f = [h.sin() * t.cos(), h.cos() * t.cos(), -t.sin()];
        let r0 = [h.cos(), -h.sin(), 0.0];
        let u0 = cross(r0, f);
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let r = [0, 1, 2].map(|k| r0[k] * cr + u0[k] * sr);
        let u = [0, 1, 2].map(|k| u0[k] * cr - r0[k] * sr);
        let w = enu(bearing, el);
        let az = dot(w, r).atan2(dot(w, f)).to_degrees().rem_euclid(360.0);
        (az, dot(w, u).clamp(-1.0, 1.0).asin().to_degrees())
    }
}

impl Pattern {
    pub fn from_fn(
        name: impl Into<String>,
        freq_mhz: Option<f64>,
        absolute: bool,
        mirror_below: bool,
        gain: impl Fn(f64, f64) -> f64 + Sync,
    ) -> Pattern {
        let db = (0..EL * AZ)
            .into_par_iter()
            .map(|i| {
                let (e, a) = (i / AZ, i % AZ);
                let v = gain(a as f64, e as f64 - 90.0);
                (if v.is_finite() { v } else { -100.0 }).max(-100.0) as f32
            })
            .collect();
        Pattern { name: name.into(), freq_mhz, absolute, mirror_below, db }
    }

    fn cell(&self, e: usize, a: usize) -> f64 {
        self.db[e.min(EL - 1) * AZ + a % AZ] as f64
    }

    pub fn gain(&self, az: f64, el: f64) -> f64 {
        let el = if self.mirror_below { el.abs() } else { el }.clamp(-90.0, 90.0) + 90.0;
        let az = az.rem_euclid(360.0);
        let (e0, a0) = (el.floor() as usize, az.floor() as usize);
        let (te, ta) = (el - e0 as f64, az - a0 as f64);
        let lo = self.cell(e0, a0) * (1.0 - ta) + self.cell(e0, a0 + 1) * ta;
        let hi = self.cell(e0 + 1, a0) * (1.0 - ta) + self.cell(e0 + 1, a0 + 1) * ta;
        lo * (1.0 - te) + hi * te
    }

    pub fn toward(&self, mount: &Mount, bearing: f64, el: f64) -> f64 {
        let (az, e) = mount.to_antenna(bearing, el);
        self.gain(az, e)
    }

    pub fn peak(&self) -> f64 {
        self.db.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64
    }

    pub fn to_text(&self) -> String {
        let mut out = String::from("# antenna-toolbox pattern v1\n");
        out.push_str(&format!("name={}\n", self.name.replace('\n', " ")));
        if let Some(f) = self.freq_mhz {
            out.push_str(&format!("freq_mhz={f}\n"));
        }
        out.push_str(&format!("absolute={}\nmirror_below={}\n", self.absolute, self.mirror_below));
        out.push_str("# rows: elevation -90 to 90 by 1; columns: azimuth 0 to 359 clockwise from boresight; dBi\n");
        for row in self.db.chunks(AZ) {
            let line: Vec<String> = row.iter().map(|v| format!("{v:.2}")).collect();
            out.push_str(&line.join(","));
            out.push('\n');
        }
        out
    }

    pub fn from_text(text: &str) -> Result<Pattern, String> {
        let mut p = Pattern {
            name: String::new(),
            freq_mhz: None,
            absolute: true,
            mirror_below: false,
            db: Vec::with_capacity(EL * AZ),
        };
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if let Some((k, v)) = line.split_once('=') {
                match k {
                    "name" => p.name = v.to_string(),
                    "freq_mhz" => p.freq_mhz = v.parse().ok(),
                    "absolute" => p.absolute = v == "true",
                    "mirror_below" => p.mirror_below = v == "true",
                    _ => {}
                }
                continue;
            }
            let row: Result<Vec<f32>, _> = line.split(',').map(|v| v.trim().parse()).collect();
            let row = row.map_err(|e| format!("bad pattern row: {e}"))?;
            if row.len() != AZ {
                return Err(format!("pattern rows need {AZ} values, found {}", row.len()));
            }
            p.db.extend(row);
        }
        if p.db.len() != EL * AZ {
            return Err(format!("pattern needs {EL} rows, found {}", p.db.len() / AZ));
        }
        Ok(p)
    }

    pub fn from_nec_output(name: &str, text: &str) -> Result<Pattern, String> {
        let block = text
            .split("RADIATION PATTERNS")
            .nth(1)
            .ok_or("no RADIATION PATTERNS table in this NEC output")?;
        let freq_mhz = text
            .split("FREQUENCY=")
            .nth(1)
            .or_else(|| text.split("FREQUENCY :").nth(1))
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<f64>().ok());
        let mut pts: Vec<(V3, f64)> = Vec::new();
        for line in block.lines().skip(5) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 5 {
                if pts.is_empty() {
                    continue;
                }
                break;
            }
            let (Ok(theta), Ok(phi), Ok(total)) =
                (f[0].parse::<f64>(), f[1].parse::<f64>(), f[4].parse::<f64>())
            else {
                break;
            };
            let (t, p) = (theta.to_radians(), phi.to_radians());
            pts.push(([t.sin() * p.cos(), t.sin() * p.sin(), t.cos()], total));
        }
        if pts.len() < 3 {
            return Err("the NEC output has fewer than three pattern points".into());
        }
        let below = pts.iter().any(|(d, _)| d[2] < -0.05);
        Ok(Pattern::from_fn(name, freq_mhz, true, !below, |az, el| {
            let (a, e) = ((-az).to_radians(), el.to_radians());
            let d = [e.cos() * a.cos(), e.cos() * a.sin(), e.sin()];
            pts.iter()
                .max_by(|x, y| dot(x.0, d).total_cmp(&dot(y.0, d)))
                .map(|x| x.1)
                .unwrap_or(-100.0)
        }))
    }

    pub fn from_splat(name: &str, az_text: &str, el_text: Option<&str>) -> Result<Pattern, String> {
        let table = |text: &str| -> Result<(Vec<f64>, Cut), String> {
            let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
            let head: Vec<f64> = lines
                .next()
                .ok_or("empty SPLAT! pattern file")?
                .split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect();
            let rows = lines
                .filter_map(|l| {
                    let mut it = l.split_whitespace().filter_map(|v| v.parse::<f64>().ok());
                    Some((it.next()?, it.next()?))
                })
                .collect::<Vec<_>>();
            Ok((head, rows))
        };
        let (az_head, az_rows) = table(az_text)?;
        if az_rows.len() < 2 {
            return Err("the .az file has no pattern rows".into());
        }
        let rotation = az_head.first().copied().unwrap_or(0.0);
        let el = el_text.map(table).transpose()?;
        let lookup = |rows: &[(f64, f64)], x: f64, wrap: bool| -> f64 {
            let mut best = (f64::INFINITY, 1.0);
            for &(a, v) in rows {
                let mut d = (a - x).abs();
                if wrap {
                    d = d.min(360.0 - d);
                }
                if d < best.0 {
                    best = (d, v);
                }
            }
            best.1
        };
        Ok(Pattern::from_fn(name, None, false, false, |az, e| {
            let field_az = lookup(&az_rows, (az - rotation).rem_euclid(360.0), true);
            let field_el = match &el {
                Some((_, rows)) if !rows.is_empty() => lookup(rows, -e, false),
                _ => 1.0,
            };
            20.0 * (field_az * field_el).max(1e-5).log10()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_and_tilt_turn_the_boresight() {
        let m = Mount { heading: 90.0, tilt: 0.0, roll: 0.0 };
        let (az, el) = m.to_antenna(90.0, 0.0);
        assert!(az.abs() < 1e-9 && el.abs() < 1e-9);
        let (az, _) = m.to_antenna(180.0, 0.0);
        assert!((az - 90.0).abs() < 1e-9, "{az}");
        let down = Mount { heading: 0.0, tilt: 5.0, roll: 0.0 };
        let (_, el) = down.to_antenna(0.0, -5.0);
        assert!(el.abs() < 1e-9, "{el}");
        let rolled = Mount { heading: 0.0, tilt: 0.0, roll: 90.0 };
        let (az, el) = rolled.to_antenna(0.0, 90.0);
        assert!(
            (az - 90.0).abs() < 1e-6 && el.abs() < 1e-6,
            "straight up is off the side: {az} {el}"
        );
    }

    #[test]
    fn a_pattern_round_trips_through_text_and_interpolates() {
        let p = Pattern::from_fn("test", Some(162.0), true, false, |az, el| {
            if az < 180.0 { 6.0 - el.abs() / 10.0 } else { -10.0 }
        });
        let back = Pattern::from_text(&p.to_text()).unwrap();
        assert_eq!(back.name, "test");
        assert_eq!(back.freq_mhz, Some(162.0));
        assert!((back.gain(10.5, 0.0) - 6.0).abs() < 0.01);
        assert!((back.gain(10.0, 20.0) - 4.0).abs() < 0.01);
        assert!((back.gain(270.0, 0.0) + 10.0).abs() < 0.01);
    }

    #[test]
    fn splat_files_multiply_the_two_cuts() {
        let az = "90.0\n0 1.0\n90 0.5\n180 0.1\n270 0.5\n";
        let el = "0.0 0.0\n-10 0.5\n0 1.0\n10 0.5\n";
        let p = Pattern::from_splat("kvea", az, Some(el)).unwrap();
        assert!(p.gain(90.0, 0.0).abs() < 0.01);
        assert!((p.gain(180.0, 0.0) + 6.02).abs() < 0.05);
        assert!((p.gain(90.0, 10.0) + 6.02).abs() < 0.05);
        assert!(!p.absolute);
    }
}
