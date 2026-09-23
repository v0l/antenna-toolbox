struct Tri { a: vec3f, area: f32, b: vec3f, pad0: f32, c: vec3f, pad1: f32,
             centre: vec3f, pad2: f32, normal: vec3f, pad3: f32 };
struct Edge { len: f32, plus: u32, minus: u32, free_plus: u32,
              free_minus: u32, pad0: u32, pad1: u32, pad2: u32 };
struct U { n: u32, w: u32, col: u32, pad: u32, k: f32, eta: f32, pad2: f32, pad3: f32 };

@group(0) @binding(0) var<storage, read> tris: array<Tri>;
@group(0) @binding(1) var<storage, read> edges: array<Edge>;
@group(0) @binding(2) var<storage, read> verts: array<vec4f>;
@group(0) @binding(3) var<storage, read_write> A: array<vec2f>;
@group(0) @binding(4) var<uniform> u: U;

const PI = 3.14159265358979;

var<private> QW = array<f32, 7>(0.225, 0.132394152788506, 0.132394152788506, 0.132394152788506,
                                0.125939180544827, 0.125939180544827, 0.125939180544827);
var<private> QA = array<f32, 7>(0.333333333333333, 0.059715871789770, 0.470142064105115,
                                0.470142064105115, 0.797426985353087, 0.101286507323456, 0.101286507323456);
var<private> QB = array<f32, 7>(0.333333333333333, 0.470142064105115, 0.059715871789770,
                                0.470142064105115, 0.101286507323456, 0.797426985353087, 0.101286507323456);

struct Analytic { scalar: f32, vector: vec3f };

fn analytic(t: Tri, r: vec3f) -> Analytic {
  let n = t.normal;
  let h = dot(r - t.a, n);
  let proj = r - n * h;
  let abs_h = abs(h);
  var scalar_sum = 0.0;
  var beta_sum = 0.0;
  var vec = vec3f(0.0);
  var corners = array<vec3f, 3>(t.a, t.b, t.c);
  for (var i = 0u; i < 3u; i = i + 1u) {
    let p1 = corners[i];
    let p2 = corners[(i + 1u) % 3u];
    let e = p2 - p1;
    let len = length(e);
    if (len < 1e-12) { continue; }
    let l_hat = e / len;
    let u_hat = cross(l_hat, n);
    let p0v = dot(p1 - proj, u_hat);
    let lm = dot(p1 - proj, l_hat);
    let lp = dot(p2 - proj, l_hat);
    let rm = length(p1 - r);
    let rp = length(p2 - r);
    let r0sq = p0v * p0v + h * h;
    var f2 = 0.0;
    let dm = rm + lm;
    let dp = rp + lp;
    if (dm > 1e-5 * len && dp > 1e-5 * len) { f2 = log(dp / dm); }
    scalar_sum = scalar_sum + p0v * f2;
    vec = vec + u_hat * (0.5 * (r0sq * f2 + lp * rp - lm * rm));
    if (abs_h > 1e-12) {
      beta_sum = beta_sum + atan2(p0v * lp, r0sq + abs_h * rp) - atan2(p0v * lm, r0sq + abs_h * rm);
    }
  }
  return Analytic(scalar_sum - abs_h * beta_sum, vec);
}

struct Source { scalar: vec2f, vx: vec2f, vy: vec2f, vz: vec2f };

fn source_integrals(t: Tri, free: vec3f, r: vec3f) -> Source {
  var sr = vec2f(0.0);
  var vx = vec2f(0.0);
  var vy = vec2f(0.0);
  var vz = vec2f(0.0);
  for (var q = 0u; q < 7u; q = q + 1u) {
    let la = QA[q];
    let lb = QB[q];
    let lc = 1.0 - la - lb;
    let p = t.a * la + t.b * lb + t.c * lc;
    let R = max(length(p - r), 1e-12);
    let kr = u.k * R;
    let sh = sin(kr * 0.5);
    let g = vec2f(-2.0 * sh * sh / R, -sin(kr) / R);
    let w = QW[q] * t.area / (4.0 * PI);
    sr = sr + g * w;
    let rho = p - free;
    vx = vx + g * (w * rho.x);
    vy = vy + g * (w * rho.y);
    vz = vz + g * (w * rho.z);
  }
  let an = analytic(t, r);
  let h = dot(r - t.a, t.normal);
  let proj = r - t.normal * h;
  let offset = proj - free;
  let inv = 1.0 / (4.0 * PI);
  let scalar = vec2f(sr.x + inv * an.scalar, sr.y);
  return Source(
    scalar,
    vec2f(vx.x + inv * (an.vector.x + offset.x * an.scalar), vx.y),
    vec2f(vy.x + inv * (an.vector.y + offset.y * an.scalar), vy.y),
    vec2f(vz.x + inv * (an.vector.z + offset.z * an.scalar), vz.y),
  );
}

fn cj(a: vec2f) -> vec2f { return vec2f(-a.y, a.x); }

@compute @workgroup_size(8, 8)
fn fill_rwg(@builtin(global_invocation_id) gid: vec3u) {
  let m = gid.x;
  let n = gid.y;
  if (m >= u.n || n >= u.n) { return; }
  let em = edges[m];
  let en = edges[n];
  let tm_p = tris[em.plus];
  let tm_m = tris[em.minus];
  let tn_p = tris[en.plus];
  let tn_m = tris[en.minus];
  let free_p = verts[en.free_plus].xyz;
  let free_m = verts[en.free_minus].xyz;
  let rho_p = tm_p.centre - verts[em.free_plus].xyz;
  let rho_m = verts[em.free_minus].xyz - tm_m.centre;
  let omega_mu = u.k * u.eta;
  let inv_omega_eps = u.eta / u.k;

  var z = vec2f(0.0);
  for (var side = 0u; side < 2u; side = side + 1u) {
    var obs = tm_p.centre;
    var rho = rho_p;
    var sign = -1.0;
    if (side == 1u) { obs = tm_m.centre; rho = rho_m; sign = 1.0; }
    let ip = source_integrals(tn_p, free_p, obs);
    let im = source_integrals(tn_m, free_m, obs);
    let fp = en.len / (2.0 * tn_p.area);
    let fm = -en.len / (2.0 * tn_m.area);
    var a_dot_rho = vec2f(0.0);
    a_dot_rho = a_dot_rho + (ip.vx * fp + im.vx * fm) * rho.x;
    a_dot_rho = a_dot_rho + (ip.vy * fp + im.vy * fm) * rho.y;
    a_dot_rho = a_dot_rho + (ip.vz * fp + im.vz * fm) * rho.z;
    let vec_part = cj(a_dot_rho * (omega_mu * 0.5));
    let phi = ip.scalar * (en.len / tn_p.area) + im.scalar * (-en.len / tn_m.area);
    let scalar_part = cj(phi * inv_omega_eps) * sign;
    z = z + vec_part + scalar_part;
  }
  A[m * u.w + n] = z * em.len;
  if (n == 0u) { A[m * u.w + u.n] = vec2f(0.0, 0.0); }
}
