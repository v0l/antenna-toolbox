use crate::{C64, Error, Point, Result};

pub const PROMPT: &[u8] = b"ch> ";

pub fn reply(buf: &[u8], cmd: &str) -> Option<Result<Vec<String>>> {
    if !buf.ends_with(PROMPT) {
        return None;
    }
    let text = String::from_utf8_lossy(&buf[..buf.len() - PROMPT.len()]).into_owned();
    let mut lines = text.split("\r\n");
    let echo = lines.next().unwrap_or_default();
    if echo.trim() != cmd {
        return Some(Err(Error::Protocol(format!("unexpected echo `{echo}` for `{cmd}`"))));
    }
    Some(Ok(lines.filter(|l| !l.is_empty()).map(str::to_string).collect()))
}

pub fn max_points(info: &[String]) -> usize {
    info.iter()
        .find_map(|l| {
            let at = l.find("p:")?;
            l[at + 2..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
        })
        .unwrap_or(101)
}

pub fn chunks(
    start_hz: f64,
    stop_hz: f64,
    points: usize,
    max: usize,
) -> Vec<(f64, f64, usize, usize)> {
    let points = points.max(2);
    let chunk = max.max(2);
    if points <= chunk {
        return vec![(start_hz, stop_hz, points, points)];
    }
    let step = (stop_hz - start_hz) / (points - 1) as f64;
    let mut out = Vec::new();
    let mut i = 0;
    while i < points {
        let n = chunk.min(points - i);
        let n = if n == 1 { 2 } else { n };
        let a = start_hz + step * i as f64;
        out.push((a, a + step * (n - 1) as f64, n, n.min(points - i)));
        i += n;
    }
    out
}

pub fn scan_command(start_hz: f64, stop_hz: f64, points: usize, s21: bool) -> String {
    let mask = if s21 { 7 } else { 3 };
    format!("scan {} {} {} {mask}", start_hz.round() as u64, stop_hz.round() as u64, points)
}

pub fn parse_scan(lines: &[String], s21: bool, points: usize) -> Result<Vec<Point>> {
    let pts = lines
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
        .collect::<Result<Vec<_>>>()?;
    if pts.len() != points {
        return Err(Error::Protocol(format!("asked for {points} points, got {}", pts.len())));
    }
    Ok(pts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scan_output() {
        let lines = vec!["868000000 0.123 -0.456".to_string(), "869000000 0.1 0.2".to_string()];
        let p = parse_scan(&lines, false, 2).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].freq, 868e6);
        assert_eq!(p[0].s11, C64::new(0.123, -0.456));
        assert!(p[0].s21.is_none());
    }

    #[test]
    fn waits_for_the_prompt_and_checks_the_echo() {
        assert!(reply(b"info\r\nhello", "info").is_none());
        let ok = reply(b"info\r\nNanoVNA-H4 p:401\r\nch> ", "info").unwrap().unwrap();
        assert_eq!(max_points(&ok), 401);
        assert!(reply(b"vers\r\nch> ", "info").unwrap().is_err());
    }
}
