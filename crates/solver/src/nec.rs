use crate::geometry::{Insulation, RealGround, SolveLine, WireGeometry, WireProps};
use crate::mom::segmentise;
use crate::vec::{Vec3, add, length, lerp, scale, sub};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    Mirrors,
    PhysicalOptics,
    FeedOffWire,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExportError::Mirrors => "corner reflector images have no NEC-2 equivalent",
            ExportError::PhysicalOptics => "a physical-optics reflector has no NEC-2 equivalent",
            ExportError::FeedOffWire => "the feed point does not lie on a wire",
        })
    }
}

struct Wire {
    a: Vec3,
    b: Vec3,
    segments: usize,
    radius: f64,
    props: WireProps,
}

fn on_edge(a: Vec3, b: Vec3, p: Vec3) -> Option<f64> {
    let ab = sub(b, a);
    let l = length(ab);
    if l == 0.0 {
        return None;
    }
    let t = crate::vec::dot(sub(p, a), ab) / (l * l);
    let off = length(sub(p, lerp(a, b, t)));
    (off < 1e-6 * l.max(1.0) && (-1e-9..=1.0 + 1e-9).contains(&t)).then_some(t)
}

pub fn export(
    geo: &WireGeometry,
    freq_mhz: f64,
    wire_radius: f64,
    title: &str,
) -> Result<String, ExportError> {
    if !geo.mirrors.is_empty() {
        return Err(ExportError::Mirrors);
    }
    if geo.po.is_some() {
        return Err(ExportError::PhysicalOptics);
    }
    let lam = 299_792.458 / freq_mhz;
    let target = lam / 60.0;
    let dz = geo.ground_z.unwrap_or(0.0);
    let shift = |p: Vec3| [p[0], p[1], p[2] - dz];

    let mut wires: Vec<Wire> = Vec::new();
    let mut feed_at: Option<(usize, usize)> = None;
    for line in &geo.lines {
        for pair in line.pts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let l = length(sub(b, a));
            if l < 1e-9 {
                continue;
            }
            let radius = line.rad.unwrap_or(wire_radius);
            let n = line.segments.unwrap_or(((l / target).round() as usize).max(1)).max(1);
            match on_edge(a, b, geo.feed).filter(|_| feed_at.is_none()) {
                Some(t) => {
                    let seg = l / n as f64;
                    let half = (seg / 2.0).min(t * l).min((1.0 - t) * l);
                    let dir = scale(sub(b, a), 1.0 / l);
                    let f0 = sub(geo.feed, scale(dir, half));
                    let f1 = add(geo.feed, scale(dir, half));
                    let before = length(sub(f0, a));
                    let after = length(sub(b, f1));
                    if before > 1e-9 {
                        let n = ((before / seg).round() as usize).max(1);
                        wires.push(Wire { a, b: f0, segments: n, radius, props: line.props });
                    }
                    feed_at = Some((wires.len() + 1, 1));
                    wires.push(Wire { a: f0, b: f1, segments: 1, radius, props: line.props });
                    if after > 1e-9 {
                        let n = ((after / seg).round() as usize).max(1);
                        wires.push(Wire { a: f1, b, segments: n, radius, props: line.props });
                    }
                }
                None => wires.push(Wire { a, b, segments: n, radius, props: line.props }),
            }
        }
    }
    let (tag, seg) = feed_at.ok_or(ExportError::FeedOffWire)?;

    let mut o = String::new();
    let _ = writeln!(o, "CM {title}");
    let _ = writeln!(o, "CM exported by antenna-toolbox, metres");
    if wires.iter().any(|w| w.props.insulation.is_some()) {
        let _ = writeln!(o, "CM insulation is not representable in NEC-2 and was dropped");
    }
    let _ = writeln!(o, "CE");
    let m = |v: f64| v / 1000.0;
    for (i, w) in wires.iter().enumerate() {
        let (a, b) = (shift(w.a), shift(w.b));
        let _ = writeln!(
            o,
            "GW {} {} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} {:.6}",
            i + 1,
            w.segments,
            m(a[0]),
            m(a[1]),
            m(a[2]),
            m(b[0]),
            m(b[1]),
            m(b[2]),
            m(w.radius)
        );
    }
    let _ = writeln!(o, "GE {}", if geo.ground_z.is_some() { 1 } else { 0 });
    match (geo.ground_z, geo.real_ground) {
        (Some(_), Some(g)) => {
            let _ = writeln!(o, "GN 0 0 0 0 {} {}", g.eps_r, g.sigma);
        }
        (Some(_), None) => {
            let _ = writeln!(o, "GN 1");
        }
        _ => {}
    }
    for (i, w) in wires.iter().enumerate() {
        if let Some(sigma) = w.props.conductivity {
            let _ = writeln!(o, "LD 5 {} 0 0 {sigma:e}", i + 1);
        }
    }
    let _ = writeln!(o, "EX 0 {tag} {seg} 0 1 0");
    let _ = writeln!(o, "FR 0 1 0 0 {freq_mhz} 0");
    let _ = writeln!(o, "RP 0 37 73 1000 0 0 5 5");
    let _ = writeln!(o, "EN");
    Ok(o)
}

#[derive(Clone, Default)]
pub struct Imported {
    pub geo: WireGeometry,
    pub freq_mhz: Option<f64>,
    pub warnings: Vec<String>,
    pub comments: Vec<String>,
}

#[derive(Clone)]
struct Raw {
    tag: i64,
    pts: Vec<Vec3>,
    radius: f64,
    props: WireProps,
}

fn nums(rest: &str) -> Vec<f64> {
    rest.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<f64>().unwrap_or(0.0))
        .collect()
}

fn rotate(p: Vec3, rx: f64, ry: f64, rz: f64) -> Vec3 {
    let (sx, cx) = rx.to_radians().sin_cos();
    let (sy, cy) = ry.to_radians().sin_cos();
    let (sz, cz) = rz.to_radians().sin_cos();
    let p = [p[0], p[1] * cx - p[2] * sx, p[1] * sx + p[2] * cx];
    let p = [p[0] * cy + p[2] * sy, p[1], -p[0] * sy + p[2] * cy];
    [p[0] * cz - p[1] * sz, p[0] * sz + p[1] * cz, p[2]]
}

pub fn import(deck: &str) -> Result<Imported, String> {
    let mut wires: Vec<Raw> = Vec::new();
    let mut out = Imported::default();
    let mut feed: Option<(i64, usize)> = None;
    let mut ground: Option<Option<RealGround>> = None;
    let mut scale_m = 1.0;
    for (ln, raw) in deck.lines().enumerate() {
        let line = raw.trim();
        if line.len() < 2 {
            continue;
        }
        let (card, rest) = line.split_at(2);
        let card = card.to_ascii_uppercase();
        let v = nums(rest);
        let get = |i: usize| v.get(i).copied().unwrap_or(0.0);
        match card.as_str() {
            "CM" | "CE" => out.comments.push(rest.trim().to_string()),
            "GW" => {
                let (tag, ns) = (get(0) as i64, get(1).max(1.0) as usize);
                let (a, b) = ([get(2), get(3), get(4)], [get(5), get(6), get(7)]);
                let pts = (0..=ns).map(|i| lerp(a, b, i as f64 / ns as f64)).collect();
                if get(8) == 0.0 {
                    out.warnings.push(format!(
                        "line {}: tapered wire (GC) read with the default radius",
                        ln + 1
                    ));
                }
                wires.push(Raw { tag, pts, radius: get(8), props: WireProps::default() });
            }
            "GA" => {
                let (tag, ns) = (get(0) as i64, get(1).max(1.0) as usize);
                let (r, a1, a2) = (get(2), get(3).to_radians(), get(4).to_radians());
                let pts = (0..=ns)
                    .map(|i| {
                        let a = a1 + (a2 - a1) * i as f64 / ns as f64;
                        [r * a.cos(), 0.0, r * a.sin()]
                    })
                    .collect();
                wires.push(Raw { tag, pts, radius: get(5), props: WireProps::default() });
            }
            "GH" => {
                let (tag, ns) = (get(0) as i64, get(1).max(1.0) as usize);
                let (spacing, len) = (get(2), get(3));
                let (a1, b1, a2, b2) = (get(4), get(5), get(6), get(7));
                let turns = if spacing != 0.0 { len.abs() / spacing } else { 0.0 };
                let hand = if len < 0.0 { -1.0 } else { 1.0 };
                let pts = (0..=ns)
                    .map(|i| {
                        let f = i as f64 / ns as f64;
                        let ang = hand * 2.0 * std::f64::consts::PI * turns * f;
                        let (ax, by) = (a1 + (a2 - a1) * f, b1 + (b2 - b1) * f);
                        [ax * ang.cos(), by * ang.sin(), len.abs() * f]
                    })
                    .collect();
                wires.push(Raw { tag, pts, radius: get(8), props: WireProps::default() });
            }
            "GM" => {
                let (inc, copies) = (get(0) as i64, get(1) as usize);
                let (rx, ry, rz, tx, ty, tz) = (get(2), get(3), get(4), get(5), get(6), get(7));
                let from = get(8) as i64;
                let chosen: Vec<usize> =
                    (0..wires.len()).filter(|&i| from == 0 || wires[i].tag >= from).collect();
                let apply = |p: Vec3| add(rotate(p, rx, ry, rz), [tx, ty, tz]);
                if copies == 0 {
                    for &i in &chosen {
                        wires[i].pts = wires[i].pts.iter().map(|&p| apply(p)).collect();
                    }
                } else {
                    let mut last: Vec<Raw> = chosen.iter().map(|&i| wires[i].clone()).collect();
                    for _ in 0..copies {
                        last = last
                            .into_iter()
                            .map(|w| Raw {
                                tag: if w.tag > 0 { w.tag + inc } else { 0 },
                                pts: w.pts.iter().map(|&p| apply(p)).collect(),
                                ..w
                            })
                            .collect();
                        wires.extend(last.iter().cloned());
                    }
                }
            }
            "GX" => {
                let inc = get(0) as i64;
                let flags = format!("{:03}", get(1) as i64);
                let mut current: Vec<Raw> = wires.clone();
                for (axis, flag) in [
                    (0usize, flags.as_bytes()[0]),
                    (1, flags.as_bytes()[1]),
                    (2, flags.as_bytes()[2]),
                ]
                .into_iter()
                .rev()
                {
                    if flag != b'1' {
                        continue;
                    }
                    let copies: Vec<Raw> = current
                        .iter()
                        .map(|w| Raw {
                            tag: if w.tag > 0 { w.tag + inc } else { 0 },
                            pts: w
                                .pts
                                .iter()
                                .rev()
                                .map(|&p| {
                                    let mut q = p;
                                    q[axis] = -q[axis];
                                    q
                                })
                                .collect(),
                            ..w.clone()
                        })
                        .collect();
                    current.extend(copies);
                }
                wires = current;
            }
            "GS" => scale_m *= if get(2) != 0.0 { get(2) } else { 1.0 },
            "GE" => {}
            "GN" => match get(0) as i64 {
                -1 => ground = None,
                1 => ground = Some(None),
                t => {
                    if t == 2 {
                        out.warnings.push(
                            "Sommerfeld ground (GN 2) read as the reflection-coefficient approximation".into(),
                        );
                    }
                    ground =
                        Some(Some(RealGround { eps_r: get(4).max(1.0), sigma: get(5).max(0.0) }));
                }
            },
            "EX" => {
                if get(0) as i64 != 0 {
                    out.warnings.push(format!(
                        "line {}: only voltage sources (EX 0) are supported",
                        ln + 1
                    ));
                }
                if feed.is_none() {
                    feed = Some((get(1) as i64, get(2).max(1.0) as usize));
                } else {
                    out.warnings.push("more than one source: only the first is driven".into());
                }
            }
            "LD" => match get(0) as i64 {
                5 => {
                    let (tag, sigma) = (get(1) as i64, get(4));
                    for w in wires.iter_mut().filter(|w| tag == 0 || w.tag == tag) {
                        w.props.conductivity = Some(sigma);
                    }
                }
                -1 => {}
                t => out.warnings.push(format!("line {}: load type {t} ignored", ln + 1)),
            },
            "IS" => {
                let (tag, eps, outer) = (get(1) as i64, get(4), get(6));
                for w in wires.iter_mut().filter(|w| tag == 0 || w.tag == tag) {
                    w.props.insulation =
                        Some(Insulation { eps_r: eps, inner: 0.0, outer: outer * 1000.0 });
                }
            }
            "FR" => {
                if out.freq_mhz.is_none() {
                    out.freq_mhz = Some(get(4));
                }
            }
            "EN" => break,
            "RP" | "XQ" | "NE" | "NH" | "PQ" | "PT" | "KH" | "NX" | "EK" | "GC" | "GF" | "WG" => {}
            other => out.warnings.push(format!("line {}: card {other} ignored", ln + 1)),
        }
    }
    let mm = |p: Vec3| scale(p, 1000.0 * scale_m);
    let (tag, seg) = feed.ok_or("no EX card: nothing is driven")?;
    let fed = wires
        .iter()
        .filter(|w| w.tag == tag)
        .flat_map(|w| w.pts.windows(2).map(|p| lerp(p[0], p[1], 0.5)))
        .nth(seg - 1)
        .ok_or_else(|| format!("EX refers to segment {seg} of tag {tag}, which does not exist"))?;
    out.geo.lines = wires
        .iter()
        .map(|w| SolveLine {
            pts: w.pts.iter().map(|&p| mm(p)).collect(),
            rad: Some(w.radius * 1000.0 * scale_m),
            props: w.props,
            segments: Some(1),
        })
        .collect();
    out.geo.feed = mm(fed);
    if let Some(real) = ground {
        out.geo.ground_z = Some(0.0);
        out.geo.real_ground = real;
    }
    Ok(out)
}

pub fn segment_count(geo: &WireGeometry, freq_mhz: f64) -> usize {
    segmentise(&geo.lines, 299_792.458 / freq_mhz, usize::MAX).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dipole_round_trips_through_a_deck() {
        let geo = WireGeometry::new(
            vec![vec![[-250.0, 0.0, 1000.0], [250.0, 0.0, 1000.0]]],
            [0.0, 0.0, 1000.0],
        );
        let deck = export(&geo, 300.0, 1.0, "dipole").unwrap();
        assert!(deck.contains("EX 0 2 1"));
        let back = import(&deck).unwrap();
        assert_eq!(back.freq_mhz, Some(300.0));
        assert!(length(sub(back.geo.feed, [0.0, 0.0, 1000.0])) < 1e-3);
        let span: f64 = back
            .geo
            .lines
            .iter()
            .flat_map(|l| l.pts.windows(2))
            .map(|p| length(sub(p[1], p[0])))
            .sum();
        assert!((span - 500.0).abs() < 1e-3);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    }

    #[test]
    fn reads_the_nec2_manual_cards() {
        let deck = "CM test\nCE\nGW 1 5 0 0 -0.25 0 0 0.25 0.001\nGM 1 2 0 0 0 0.1 0 0 1\nGS 0 0 2\nGE 1\nGN 0 0 0 0 13 0.005\nLD 5 0 0 0 5.8E7\nEX 0 2 3 0 1 0\nFR 0 1 0 0 146 0\nEN\n";
        let r = import(deck).unwrap();
        assert_eq!(r.geo.lines.len(), 3);
        assert_eq!(r.freq_mhz, Some(146.0));
        assert!(length(sub(r.geo.feed, [200.0, 0.0, 0.0])) < 1e-6, "{:?}", r.geo.feed);
        assert_eq!(r.geo.real_ground, Some(RealGround { eps_r: 13.0, sigma: 0.005 }));
        assert!(r.geo.lines.iter().all(|l| l.props.conductivity == Some(5.8e7)));
        assert!((r.geo.lines[0].rad.unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_helix_card_winds_the_right_number_of_turns() {
        let r = import("GH 1 120 0.1 0.5 0.05 0.05 0.05 0.05 0.001\nEX 0 1 1 0 1\nEN\n").unwrap();
        let pts = &r.geo.lines[0].pts;
        assert!((pts.last().unwrap()[2] - 500.0).abs() < 1e-6);
        let crossings = pts.windows(2).filter(|w| w[0][1] < 0.0 && w[1][1] >= 0.0).count();
        assert_eq!(crossings, 4);
    }
}
