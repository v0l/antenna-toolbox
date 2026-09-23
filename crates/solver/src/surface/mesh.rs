use crate::geometry::{Mesh, Tri};
use crate::vec::{Vec3, add, cross, dot, length, scale, sub};
use std::collections::HashMap;
use std::f64::consts::PI;

pub type Vec2 = [f64; 2];

fn cross2(o: Vec2, a: Vec2, b: Vec2) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn signed_area(poly: &[Vec2]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1])
        .sum::<f64>()
        / 2.0
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = cross2(a, b, p);
    let d2 = cross2(b, c, p);
    let d3 = cross2(c, a, p);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

pub fn triangulate(poly: &[Vec2]) -> Vec<Tri> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area(poly) < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let mut clipped = false;
        for i in 0..idx.len() {
            let ia = idx[(i + idx.len() - 1) % idx.len()];
            let ib = idx[i];
            let ic = idx[(i + 1) % idx.len()];
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross2(a, b, c) <= 0.0 {
                continue;
            }
            let contains = idx
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && point_in_triangle(poly[j], a, b, c));
            if contains {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

pub fn refine(mesh: &Mesh, max_edge: f64) -> Mesh {
    let mut vertices = mesh.vertices.clone();
    let mut triangles = mesh.triangles.clone();
    for _ in 0..8 {
        let longest = triangles.iter().fold(0.0f64, |m, t| {
            let (a, b, c) = (vertices[t[0]], vertices[t[1]], vertices[t[2]]);
            m.max(length(sub(b, a))).max(length(sub(c, b))).max(length(sub(a, c)))
        });
        if longest <= max_edge {
            break;
        }
        let mut mid: HashMap<(usize, usize), usize> = HashMap::new();
        let mut next = Vec::with_capacity(triangles.len() * 4);
        for &[a, b, c] in &triangles {
            let mut midpoint = |i: usize, j: usize| {
                let key = if i < j { (i, j) } else { (j, i) };
                *mid.entry(key).or_insert_with(|| {
                    vertices.push(scale(add(vertices[i], vertices[j]), 0.5));
                    vertices.len() - 1
                })
            };
            let ab = midpoint(a, b);
            let bc = midpoint(b, c);
            let ca = midpoint(c, a);
            next.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        }
        triangles = next;
    }
    Mesh { vertices, triangles }
}

pub fn mesh_polygon(poly: &[Vec2], z: f64, max_edge: f64) -> Mesh {
    let vertices = poly.iter().map(|p| [p[0], p[1], z]).collect();
    refine(&Mesh { vertices, triangles: triangulate(poly) }, max_edge)
}

pub fn weld(mesh: &Mesh, tol: f64) -> Mesh {
    let mut map: HashMap<[i64; 3], usize> = HashMap::new();
    let mut vertices = Vec::new();
    let remap: Vec<usize> = mesh
        .vertices
        .iter()
        .map(|p| {
            let key = p.map(|v| (v / tol).round() as i64);
            *map.entry(key).or_insert_with(|| {
                vertices.push(*p);
                vertices.len() - 1
            })
        })
        .collect();
    let triangles = mesh
        .triangles
        .iter()
        .map(|t| [remap[t[0]], remap[t[1]], remap[t[2]]])
        .filter(|[a, b, c]| a != b && b != c && c != a)
        .collect();
    Mesh { vertices, triangles }
}

pub fn merge_meshes(meshes: &[Mesh], tol: f64) -> Mesh {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for m in meshes {
        let base = vertices.len();
        vertices.extend_from_slice(&m.vertices);
        triangles.extend(m.triangles.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }
    weld(&Mesh { vertices, triangles }, tol)
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub v: Tri,
    pub centre: Vec3,
    pub area: f64,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct RwgEdge {
    pub a: usize,
    pub b: usize,
    pub length: f64,
    pub plus: usize,
    pub minus: usize,
    pub free_plus: usize,
    pub free_minus: usize,
}

#[derive(Clone, Debug)]
pub struct SurfaceTopology {
    pub mesh: Mesh,
    pub triangles: Vec<Triangle>,
    pub edges: Vec<RwgEdge>,
}

pub fn topology(mesh: &Mesh) -> SurfaceTopology {
    let triangles = mesh
        .triangles
        .iter()
        .map(|&t| {
            let (a, b, c) = (mesh.vertices[t[0]], mesh.vertices[t[1]], mesh.vertices[t[2]]);
            let n = cross(sub(b, a), sub(c, a));
            let twice = length(n);
            Triangle {
                v: t,
                area: twice / 2.0,
                normal: scale(n, 1.0 / if twice > 0.0 { twice } else { 1.0 }),
                centre: scale(add(add(a, b), c), 1.0 / 3.0),
            }
        })
        .collect();

    let mut order: Vec<(usize, usize)> = Vec::new();
    let mut shared: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (ti, t) in mesh.triangles.iter().enumerate() {
        for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = if i < j { (i, j) } else { (j, i) };
            shared
                .entry(key)
                .or_insert_with(|| {
                    order.push(key);
                    Vec::new()
                })
                .push(ti);
        }
    }

    let edges = order
        .into_iter()
        .filter_map(|key| {
            let tris = &shared[&key];
            if tris.len() != 2 {
                return None;
            }
            let (ai, bi) = key;
            let free = |ti: usize| {
                *mesh.triangles[ti].iter().find(|&&v| v != ai && v != bi).expect("free vertex")
            };
            Some(RwgEdge {
                a: ai,
                b: bi,
                length: length(sub(mesh.vertices[bi], mesh.vertices[ai])),
                plus: tris[0],
                minus: tris[1],
                free_plus: free(tris[0]),
                free_minus: free(tris[1]),
            })
        })
        .collect();
    SurfaceTopology { mesh: mesh.clone(), triangles, edges }
}

fn edge_mid(topo: &SurfaceTopology, e: &RwgEdge) -> Vec3 {
    scale(add(topo.mesh.vertices[e.a], topo.mesh.vertices[e.b]), 0.5)
}

fn edge_flow(topo: &SurfaceTopology, e: &RwgEdge) -> Vec3 {
    sub(topo.triangles[e.minus].centre, topo.triangles[e.plus].centre)
}

pub fn nearest_edge(topo: &SurfaceTopology, p: Vec3, current_dir: Option<Vec3>) -> usize {
    let dir_len = current_dir.map(length).unwrap_or(0.0);
    let mut best = 0;
    let mut best_score = f64::INFINITY;
    for (i, e) in topo.edges.iter().enumerate() {
        let d = length(sub(edge_mid(topo, e), p));
        let mut score = d;
        if let Some(cd) = current_dir.filter(|_| dir_len > 0.0) {
            let flow = edge_flow(topo, e);
            let fl = length(flow);
            let align = if fl > 0.0 { (dot(flow, cd) / (fl * dir_len)).abs() } else { 0.0 };
            score = d + (1.0 - align) * e.length * 8.0;
        }
        if score < best_score {
            best_score = score;
            best = i;
        }
    }
    best
}

pub fn feed_edges(topo: &SurfaceTopology, p: Vec3, current_dir: Vec3, tol: f64) -> Vec<usize> {
    let dl = length(current_dir);
    let dir = scale(current_dir, 1.0 / if dl > 0.0 { dl } else { 1.0 });
    let picked: Vec<usize> = topo
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if dot(sub(edge_mid(topo, e), p), dir).abs() > tol {
                return false;
            }
            let flow = edge_flow(topo, e);
            let fl = length(flow);
            fl != 0.0 && dot(flow, dir).abs() / fl >= 0.7
        })
        .map(|(i, _)| i)
        .collect();
    if picked.is_empty() { vec![nearest_edge(topo, p, Some(current_dir))] } else { picked }
}

fn grid_triangles(nx: usize, ny: usize, at: impl Fn(usize, usize) -> usize) -> Vec<Tri> {
    let mut triangles = Vec::with_capacity(nx * ny * 2);
    for i in 0..nx {
        for j in 0..ny {
            if (i + j) % 2 == 0 {
                triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            } else {
                triangles.push([at(i, j), at(i + 1, j), at(i, j + 1)]);
                triangles.push([at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
    }
    triangles
}

pub fn mesh_strip(
    x0: f64,
    x1: f64,
    stations: usize,
    y_low: impl Fn(f64) -> f64,
    y_high: impl Fn(f64) -> f64,
    z: f64,
    across: usize,
) -> Mesh {
    let nx = stations.max(2);
    let ny = across.max(1);
    let mut vertices = Vec::with_capacity((nx + 1) * (ny + 1));
    for i in 0..=nx {
        let x = x0 + (x1 - x0) * i as f64 / nx as f64;
        let (lo, hi) = (y_low(x), y_high(x));
        for j in 0..=ny {
            vertices.push([x, lo + (hi - lo) * j as f64 / ny as f64, z]);
        }
    }
    Mesh { vertices, triangles: grid_triangles(nx, ny, |i, j| i * (ny + 1) + j) }
}

pub fn mesh_profile(
    x0: f64,
    x1: f64,
    stations: usize,
    half_height: impl Fn(f64) -> f64,
    z: f64,
    across: usize,
) -> Mesh {
    mesh_strip(x0, x1, stations, |x| -half_height(x), &half_height, z, across)
}

pub fn mesh_rect(x0: f64, x1: f64, y0: f64, y1: f64, z: f64, cell: f64, cell_y: f64) -> Mesh {
    let nx = (((x1 - x0).abs() / cell).round() as usize).max(1);
    let ny = (((y1 - y0).abs() / cell_y).round() as usize).max(1);
    let mut vertices = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            vertices.push([
                x0 + (x1 - x0) * i as f64 / nx as f64,
                y0 + (y1 - y0) * j as f64 / ny as f64,
                z,
            ]);
        }
    }
    let triangles = {
        let at = |i: usize, j: usize| j * (nx + 1) + i;
        let mut t = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                if (i + j) % 2 == 0 {
                    t.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                    t.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
                } else {
                    t.push([at(i, j), at(i + 1, j), at(i, j + 1)]);
                    t.push([at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
                }
            }
        }
        t
    };
    Mesh { vertices, triangles }
}

pub fn mesh_paraboloid(diameter: f64, focal: f64, cell: f64, apex_z: f64) -> Mesh {
    let r_max = diameter / 2.0;
    let rings = ((r_max / cell).round() as usize).max(3);
    let mut vertices = vec![[0.0, 0.0, apex_z]];
    let mut ring_start = vec![0usize];
    let mut ring_count = vec![1usize];
    for i in 1..=rings {
        let r = r_max * i as f64 / rings as f64;
        let sectors = ((2.0 * PI * r / cell).round() as usize).max(6);
        ring_start.push(vertices.len());
        ring_count.push(sectors);
        for j in 0..sectors {
            let th = j as f64 / sectors as f64 * 2.0 * PI;
            vertices.push([r * th.cos(), r * th.sin(), apex_z + r * r / (4.0 * focal)]);
        }
    }
    let mut triangles = Vec::new();
    for j in 0..ring_count[1] {
        triangles.push([0, ring_start[1] + j, ring_start[1] + (j + 1) % ring_count[1]]);
    }
    for i in 1..rings {
        let inner = ring_count[i];
        let outer = ring_count[i + 1];
        for j in 0..outer {
            let o0 = ring_start[i + 1] + j;
            let o1 = ring_start[i + 1] + (j + 1) % outer;
            let ia = ring_start[i] + (j * inner / outer) % inner;
            let ib = ring_start[i] + ((j + 1) * inner / outer) % inner;
            triangles.push([ia, o0, o1]);
            if ia != ib {
                triangles.push([ia, o1, ib]);
            }
        }
    }
    Mesh { vertices, triangles }
}
