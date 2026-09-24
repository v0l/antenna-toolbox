struct Params {
    n0: u32,
    n1: u32,
    n2: u32,
    nn: u32,
    pml: u32,
    port_count: u32,
    port_axis: u32,
    patches: u32,
    psi: array<vec4<u32>, 5>,
    dt: f32,
    k_mu: f32,
    t0: f32,
    tau: f32,
    w0: f32,
    wf: f32,
    series_cap: u32,
    pad: u32,
    cur: vec4<u32>,
    cur_d: vec4<f32>,
}

@group(0) @binding(0) var<storage, read_write> fields: array<f32>;
@group(0) @binding(1) var<storage, read> coef: array<f32>;
@group(0) @binding(2) var<storage, read_write> psi: array<f32>;
@group(0) @binding(3) var<storage, read> axes: array<f32>;
@group(0) @binding(4) var<uniform> P: Params;
@group(0) @binding(5) var<storage, read_write> state: array<u32>;
@group(0) @binding(6) var<storage, read_write> series: array<f32>;
@group(0) @binding(7) var<storage, read> port: array<f32>;

@group(1) @binding(0) var<storage, read> patch_idx: array<u32>;
@group(1) @binding(1) var<storage, read_write> acc: array<f32>;
@group(1) @binding(2) var<storage, read_write> partial: array<f32>;

const STRIDE: u32 = 10u;

fn dim(a: u32) -> u32 {
    if a == 0u { return P.n0; }
    if a == 1u { return P.n1; }
    return P.n2;
}

fn base(a: u32) -> u32 {
    if a == 0u { return 0u; }
    if a == 1u { return P.n0; }
    return P.n0 + P.n1;
}

fn ax(a: u32, i: u32, f: u32) -> f32 {
    return axes[(base(a) + i) * STRIDE + f];
}

fn slot(a: u32, i: u32, f: u32) -> i32 {
    return i32(ax(a, i, f));
}

fn idx(p: vec3<u32>) -> u32 {
    return (p.x * P.n1 + p.y) * P.n2 + p.z;
}

fn psi_base(e: u32, c: u32, a: u32) -> u32 {
    let k = e * 9u + c * 3u + a;
    return P.psi[k / 4u][k % 4u];
}

fn psi_idx(a: u32, p: vec3<u32>, s: u32) -> u32 {
    let w = 2u * P.pml;
    if a == 0u { return (s * P.n1 + p.y) * P.n2 + p.z; }
    if a == 1u { return (p.x * w + s) * P.n2 + p.z; }
    return (p.x * P.n1 + p.y) * w + s;
}

fn comp(p: vec3<u32>, a: u32) -> u32 {
    if a == 0u { return p.x; }
    if a == 1u { return p.y; }
    return p.z;
}

fn step_along(p: vec3<u32>, a: u32, d: i32) -> vec3<u32> {
    var q = vec3<i32>(p);
    if a == 0u { q.x += d; }
    if a == 1u { q.y += d; }
    if a == 2u { q.z += d; }
    return vec3<u32>(q);
}

@compute @workgroup_size(64, 4, 1)
fn update_h(@builtin(global_invocation_id) g: vec3<u32>) {
    let p = vec3<u32>(g.z, g.y, g.x);
    if p.x >= P.n0 || p.y >= P.n1 || p.z >= P.n2 { return; }
    let id = idx(p);
    for (var c = 0u; c < 3u; c++) {
        let u = (c + 1u) % 3u;
        let v = (c + 2u) % 3u;
        let pu = comp(p, u);
        let pv = comp(p, v);
        if pu + 1u >= dim(u) || pv + 1u >= dim(v) { continue; }
        let ev = fields[v * P.nn + idx(step_along(p, u, 1))] - fields[v * P.nn + id];
        let eu = fields[u * P.nn + idx(step_along(p, v, 1))] - fields[u * P.nn + id];
        var t1 = ev * ax(u, pu, 0u);
        var t2 = eu * ax(v, pv, 0u);
        if P.pml > 0u {
            let su = slot(u, pu, 9u);
            if su >= 0 {
                let q = psi_base(1u, c, u) + psi_idx(u, p, u32(su));
                psi[q] = ax(u, pu, 6u) * psi[q] + ax(u, pu, 7u) * ev * ax(u, pu, 2u);
                t1 += psi[q];
            }
            let sv = slot(v, pv, 9u);
            if sv >= 0 {
                let q = psi_base(1u, c, v) + psi_idx(v, p, u32(sv));
                psi[q] = ax(v, pv, 6u) * psi[q] + ax(v, pv, 7u) * eu * ax(v, pv, 2u);
                t2 += psi[q];
            }
        }
        fields[(3u + c) * P.nn + id] -= P.k_mu * (t1 - t2);
    }
}

@compute @workgroup_size(64, 4, 1)
fn update_e(@builtin(global_invocation_id) g: vec3<u32>) {
    let p = vec3<u32>(g.z, g.y, g.x);
    if p.x >= P.n0 || p.y >= P.n1 || p.z >= P.n2 { return; }
    let id = idx(p);
    for (var c = 0u; c < 3u; c++) {
        let cb = coef[(3u + c) * P.nn + id];
        if cb == 0.0 { continue; }
        let u = (c + 1u) % 3u;
        let v = (c + 2u) % 3u;
        let pu = comp(p, u);
        let pv = comp(p, v);
        let hv = fields[(3u + v) * P.nn + id] - fields[(3u + v) * P.nn + idx(step_along(p, u, -1))];
        let hu = fields[(3u + u) * P.nn + id] - fields[(3u + u) * P.nn + idx(step_along(p, v, -1))];
        var t1 = hv * ax(u, pu, 1u);
        var t2 = hu * ax(v, pv, 1u);
        if P.pml > 0u {
            let su = slot(u, pu, 8u);
            if su >= 0 {
                let q = psi_base(0u, c, u) + psi_idx(u, p, u32(su));
                psi[q] = ax(u, pu, 4u) * psi[q] + ax(u, pu, 5u) * hv * ax(u, pu, 3u);
                t1 += psi[q];
            }
            let sv = slot(v, pv, 8u);
            if sv >= 0 {
                let q = psi_base(0u, c, v) + psi_idx(v, p, u32(sv));
                psi[q] = ax(v, pv, 4u) * psi[q] + ax(v, pv, 5u) * hu * ax(v, pv, 3u);
                t2 += psi[q];
            }
        }
        let f = c * P.nn + id;
        fields[f] = coef[f] * fields[f] + cb * (t1 - t2);
    }
}

fn pulse(t: f32) -> f32 {
    let x = (t - P.t0) / P.tau;
    return exp(-x * x) * cos(P.w0 * (t - P.t0));
}

@compute @workgroup_size(64, 1, 1)
fn source(@builtin(global_invocation_id) g: vec3<u32>) {
    if g.x >= P.port_count { return; }
    let n = state[0];
    let t = (f32(n) + 0.5) * P.dt;
    let vs = pulse(t) / f32(P.port_count);
    let id = u32(port[g.x * 3u]);
    fields[P.port_axis * P.nn + id] -= port[g.x * 3u + 1u] * vs;
}

@compute @workgroup_size(1, 1, 1)
fn probe() {
    let n = state[0];
    var v = 0.0;
    for (var e = 0u; e < P.port_count; e++) {
        let id = u32(port[e * 3u]);
        v -= fields[P.port_axis * P.nn + id] * port[e * 3u + 2u];
    }
    let hv = 3u + P.cur.w / 4u;
    let hu = 3u + P.cur.w % 4u;
    let i = (fields[hv * P.nn + P.cur.x] - fields[hv * P.nn + P.cur.y]) * P.cur_d.x
        - (fields[hu * P.nn + P.cur.x] - fields[hu * P.nn + P.cur.z]) * P.cur_d.y;
    if n < P.series_cap {
        series[2u * n] = v;
        series[2u * n + 1u] = i;
    }
    state[0] = n + 1u;
}

@compute @workgroup_size(64, 1, 1)
fn ntff(@builtin(global_invocation_id) g: vec3<u32>) {
    if g.x >= P.patches { return; }
    let n = state[0];
    let b = g.x * 13u;
    let meta_ = patch_idx[b];
    let axis = meta_ & 3u;
    let side = select(-1.0, 1.0, (meta_ & 4u) != 0u);
    let u = (axis + 1u) % 3u;
    let v = (axis + 2u) % 3u;
    var ef = array<f32, 3>(0.0, 0.0, 0.0);
    var hf = array<f32, 3>(0.0, 0.0, 0.0);
    ef[u] = 0.5 * (fields[u * P.nn + patch_idx[b + 1u]] + fields[u * P.nn + patch_idx[b + 2u]]);
    ef[v] = 0.5 * (fields[v * P.nn + patch_idx[b + 3u]] + fields[v * P.nn + patch_idx[b + 4u]]);
    hf[u] = 0.25 * (fields[(3u + u) * P.nn + patch_idx[b + 5u]] + fields[(3u + u) * P.nn + patch_idx[b + 6u]]
        + fields[(3u + u) * P.nn + patch_idx[b + 7u]] + fields[(3u + u) * P.nn + patch_idx[b + 8u]]);
    hf[v] = 0.25 * (fields[(3u + v) * P.nn + patch_idx[b + 9u]] + fields[(3u + v) * P.nn + patch_idx[b + 10u]]
        + fields[(3u + v) * P.nn + patch_idx[b + 11u]] + fields[(3u + v) * P.nn + patch_idx[b + 12u]]);
    var nrm = array<f32, 3>(0.0, 0.0, 0.0);
    nrm[axis] = side;
    let j = vec3<f32>(nrm[1] * hf[2] - nrm[2] * hf[1], nrm[2] * hf[0] - nrm[0] * hf[2], nrm[0] * hf[1] - nrm[1] * hf[0]);
    let m = vec3<f32>(ef[1] * nrm[2] - ef[2] * nrm[1], ef[2] * nrm[0] - ef[0] * nrm[2], ef[0] * nrm[1] - ef[1] * nrm[0]);
    let te = (f32(n) + 1.0) * P.dt;
    let th = (f32(n) + 0.5) * P.dt;
    let pe = vec2<f32>(cos(-P.wf * te), sin(-P.wf * te)) * P.dt;
    let ph = vec2<f32>(cos(-P.wf * th), sin(-P.wf * th)) * P.dt;
    let o = g.x * 12u;
    for (var c = 0u; c < 3u; c++) {
        acc[o + 2u * c] += ph.x * j[c];
        acc[o + 2u * c + 1u] += ph.y * j[c];
        acc[o + 6u + 2u * c] += pe.x * m[c];
        acc[o + 6u + 2u * c + 1u] += pe.y * m[c];
    }
}

var<workgroup> scratch: array<f32, 256>;

@compute @workgroup_size(256, 1, 1)
fn energy(@builtin(global_invocation_id) g: vec3<u32>, @builtin(local_invocation_id) l: vec3<u32>, @builtin(workgroup_id) w: vec3<u32>) {
    var s = 0.0;
    let total = 3u * P.nn;
    let stride = 256u * 1024u;
    var i = w.x * 256u + l.x;
    while i < total {
        let x = fields[i];
        s += x * x;
        i += stride;
    }
    scratch[l.x] = s;
    workgroupBarrier();
    for (var k = 128u; k > 0u; k /= 2u) {
        if l.x < k { scratch[l.x] += scratch[l.x + k]; }
        workgroupBarrier();
    }
    if l.x == 0u { partial[w.x] = scratch[0]; }
}
