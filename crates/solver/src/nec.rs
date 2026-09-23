use crate::C64;
use crate::geometry::{Insulation, Load, Network, RealGround, SolveLine, WireGeometry, WireProps};
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

    #[derive(Clone, Copy)]
    enum Port {
        Feed,
        Source(C64),
        Load(Load),
        Net(usize, usize),
    }
    let mut ports: Vec<(Vec3, Port)> = vec![(geo.feed, Port::Feed)];
    ports.extend(geo.sources.iter().map(|&(p, v)| (p, Port::Source(v))));
    ports.extend(geo.loads.iter().map(|&(p, l)| (p, Port::Load(l))));
    for (i, n) in geo.networks.iter().enumerate() {
        ports.push((n.0, Port::Net(i, 0)));
        ports.push((n.1, Port::Net(i, 1)));
    }
    let mut placed = vec![false; ports.len()];
    let mut wires: Vec<Wire> = Vec::new();
    let mut tagged: Vec<(usize, Port)> = Vec::new();
    for line in &geo.lines {
        for pair in line.pts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let l = length(sub(b, a));
            if l < 1e-9 {
                continue;
            }
            let radius = line.rad.unwrap_or(wire_radius);
            let n = line.segments.unwrap_or(((l / target).round() as usize).max(1)).max(1);
            let seg = l / n as f64;
            let half = (seg / 2.0).min(l / 2.0);
            let dir = scale(sub(b, a), 1.0 / l);
            let mut here: Vec<(f64, usize)> = ports
                .iter()
                .enumerate()
                .filter(|(i, _)| !placed[*i])
                .filter_map(|(i, (p, _))| {
                    on_edge(a, b, *p).map(|t| (t.clamp(half / l, 1.0 - half / l), i))
                })
                .collect();
            here.sort_by(|x, y| x.0.total_cmp(&y.0));
            let mut start = a;
            for (t, i) in here {
                let centre = lerp(a, b, t);
                let f0 = sub(centre, scale(dir, half));
                let f1 = add(centre, scale(dir, half));
                if crate::vec::dot(sub(f0, start), dir) < -1e-9 {
                    continue;
                }
                placed[i] = true;
                let before = length(sub(f0, start));
                if before > 1e-9 {
                    let n = ((before / seg).round() as usize).max(1);
                    wires.push(Wire { a: start, b: f0, segments: n, radius, props: line.props });
                }
                tagged.push((wires.len() + 1, ports[i].1));
                wires.push(Wire { a: f0, b: f1, segments: 1, radius, props: line.props });
                start = f1;
            }
            let rest = length(sub(b, start));
            if rest > 1e-9 {
                let n = ((rest / seg).round() as usize).max(1);
                wires.push(Wire { a: start, b, segments: n, radius, props: line.props });
            }
        }
    }
    if !placed[0] {
        return Err(ExportError::FeedOffWire);
    }

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
    for (tag, port) in &tagged {
        match port {
            Port::Feed => {
                let _ = writeln!(o, "EX 0 {tag} 1 0 1 0");
            }
            Port::Source(v) => {
                let _ = writeln!(o, "EX 0 {tag} 1 0 {} {}", v.re, v.im);
            }
            Port::Load(Load::Series { r, l, c }) => {
                let _ = writeln!(o, "LD 0 {tag} 1 1 {r:e} {l:e} {c:e}");
            }
            Port::Load(Load::Parallel { r, l, c }) => {
                let _ = writeln!(o, "LD 1 {tag} 1 1 {r:e} {l:e} {c:e}");
            }
            Port::Load(Load::Impedance { r, x }) => {
                let _ = writeln!(o, "LD 4 {tag} 1 1 {r:e} {x:e}");
            }
            Port::Net(..) => {}
        }
    }
    for (i, (_, _, net)) in geo.networks.iter().enumerate() {
        let end = |e: usize| {
            tagged.iter().find(|t| matches!(t.1, Port::Net(j, k) if j == i && k == e)).map(|t| t.0)
        };
        let (Some(t1), Some(t2)) = (end(0), end(1)) else {
            continue;
        };
        match *net {
            Network::Line { z0, length, crossed, shunt } => {
                let z = if crossed { -z0 } else { z0 };
                let _ = writeln!(
                    o,
                    "TL {t1} 1 {t2} 1 {z} {:.6} {:e} {:e} {:e} {:e}",
                    length / 1000.0,
                    shunt[0].re,
                    shunt[0].im,
                    shunt[1].re,
                    shunt[1].im
                );
            }
            Network::Admittance { y11, y12, y22 } => {
                let _ = writeln!(
                    o,
                    "NT {t1} 1 {t2} 1 {:e} {:e} {:e} {:e} {:e} {:e}",
                    y11.re, y11.im, y12.re, y12.im, y22.re, y22.im
                );
            }
        }
    }
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
    taper: Option<(f64, f64)>,
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
    let mut sources: Vec<(i64, usize, C64)> = Vec::new();
    let mut loads: Vec<(i64, i64, usize, usize, [f64; 3])> = Vec::new();
    let mut nets: Vec<(bool, [i64; 4], [f64; 6])> = Vec::new();
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
                wires.push(Raw {
                    tag,
                    pts,
                    radius: get(8),
                    taper: None,
                    props: WireProps::default(),
                });
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
                wires.push(Raw {
                    tag,
                    pts,
                    radius: get(5),
                    taper: None,
                    props: WireProps::default(),
                });
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
                wires.push(Raw {
                    tag,
                    pts,
                    radius: get(8),
                    taper: None,
                    props: WireProps::default(),
                });
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
                let v = C64::new(get(4), get(5));
                let v = if v.norm() == 0.0 { C64::new(1.0, 0.0) } else { v };
                sources.push((get(1) as i64, get(2).max(1.0) as usize, v));
            }
            "LD" => match get(0) as i64 {
                5 => {
                    let (tag, sigma) = (get(1) as i64, get(4));
                    for w in wires.iter_mut().filter(|w| tag == 0 || w.tag == tag) {
                        w.props.conductivity = Some(sigma);
                    }
                }
                -1 => loads.clear(),
                t @ (0 | 1 | 4) => loads.push((
                    t,
                    get(1) as i64,
                    get(2) as usize,
                    get(3) as usize,
                    [get(4), get(5), get(6)],
                )),
                t => out.warnings.push(format!("line {}: load type {t} ignored", ln + 1)),
            },
            "IS" => {
                let (tag, eps, outer) = (get(1) as i64, get(4), get(6));
                for w in wires.iter_mut().filter(|w| tag == 0 || w.tag == tag) {
                    w.props.insulation =
                        Some(Insulation { eps_r: eps, inner: 0.0, outer: outer * 1000.0 });
                }
            }
            "TL" | "NT" => {
                let ends = [get(0) as i64, get(1) as i64, get(2) as i64, get(3) as i64];
                let f = [get(4), get(5), get(6), get(7), get(8), get(9)];
                if ends[0] == -1 {
                    nets.clear();
                } else {
                    nets.push((card == "TL", ends, f));
                }
            }
            "FR" => {
                if out.freq_mhz.is_none() {
                    out.freq_mhz = Some(get(4));
                }
            }
            "EN" => break,
            "GC" => {
                if let Some(w) = wires.last_mut() {
                    w.taper = Some((get(3), get(4)));
                    if w.radius == 0.0 {
                        w.radius = (get(3) + get(4)) / 2.0;
                    }
                }
            }
            "RP" | "XQ" | "NE" | "NH" | "PQ" | "PT" | "KH" | "NX" | "EK" | "GF" | "WG" => {}
            other => out.warnings.push(format!("line {}: card {other} ignored", ln + 1)),
        }
    }
    let mm = |p: Vec3| scale(p, 1000.0 * scale_m);
    let centre = |tag: i64, seg: usize| -> Result<Vec3, String> {
        wires
            .iter()
            .filter(|w| w.tag == tag)
            .flat_map(|w| w.pts.windows(2).map(|p| lerp(p[0], p[1], 0.5)))
            .nth(seg.max(1) - 1)
            .map(mm)
            .ok_or_else(|| format!("segment {seg} of tag {tag} does not exist"))
    };
    let (&(tag, seg, v0), rest) = sources.split_first().ok_or("no EX card: nothing is driven")?;
    let fed = centre(tag, seg)?;
    for &(t, s, v) in rest {
        out.geo.sources.push((centre(t, s)?, v / v0));
    }
    for &(line, [t1, s1, t2, s2], f) in &nets {
        let (a, b) = (centre(t1, s1 as usize)?, centre(t2, s2 as usize)?);
        let net = if line {
            let length = if f[1] > 0.0 { f[1] * 1000.0 } else { length(sub(b, a)) };
            Network::Line {
                z0: f[0].abs(),
                length,
                crossed: f[0] < 0.0,
                shunt: [C64::new(f[2], f[3]), C64::new(f[4], f[5])],
            }
        } else {
            Network::Admittance {
                y11: C64::new(f[0], f[1]),
                y12: C64::new(f[2], f[3]),
                y22: C64::new(f[4], f[5]),
            }
        };
        out.geo.networks.push((a, b, net));
    }
    for &(kind, tag, s1, s2, [a, b, c]) in &loads {
        let load = match kind {
            0 => Load::Series { r: a, l: b, c },
            1 => Load::Parallel { r: a, l: b, c },
            _ => Load::Impedance { r: a, x: b },
        };
        for w in wires.iter().filter(|w| tag == 0 || w.tag == tag) {
            let n = w.pts.len() - 1;
            let (lo, hi) =
                if s1 == 0 { (1, n) } else { (s1, if s2 == 0 { s1 } else { s2 }.min(n)) };
            for s in lo..=hi {
                out.geo.loads.push((mm(lerp(w.pts[s - 1], w.pts[s], 0.5)), load));
            }
        }
    }
    out.geo.lines = wires
        .iter()
        .flat_map(|w| {
            let n = w.pts.len() - 1;
            let lines: Vec<SolveLine> = match w.taper {
                Some((r1, r2)) => (0..n)
                    .map(|i| SolveLine {
                        pts: vec![mm(w.pts[i]), mm(w.pts[i + 1])],
                        rad: Some(
                            (r1 + (r2 - r1) * (i as f64 + 0.5) / n as f64) * 1000.0 * scale_m,
                        ),
                        props: w.props,
                        segments: Some(1),
                    })
                    .collect(),
                None => vec![SolveLine {
                    pts: w.pts.iter().map(|&p| mm(p)).collect(),
                    rad: Some(w.radius * 1000.0 * scale_m),
                    props: w.props,
                    segments: Some(1),
                }],
            };
            lines
        })
        .collect();
    out.geo.feed = fed;
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
