use antenna_solver::fdtd::{
    Aabb, Dielectric, Model, Options, Outcome, Part, Port, Stuff, Triangle, gpu, ntff, run, run_gpu,
};
use std::f64::consts::PI;

fn dipole() -> Model {
    let (len, g) = (0.141, 0.003);
    Model {
        parts: Vec::new(),
        metal: vec![
            Aabb::new([0.0, 0.0, -len / 2.0], [0.0, 0.0, -g / 2.0]),
            Aabb::new([0.0, 0.0, g / 2.0], [0.0, 0.0, len / 2.0]),
        ],
        dielectrics: vec![],
        port: Some(Port { a: [0.0, 0.0, -g / 2.0], b: [0.0, 0.0, g / 2.0], r: 50.0 }),
        f0: 1.0e9,
        span: 0.2,
        fine: g,
        ground_plane_z: None,
    }
}

fn radiated(out: &Outcome) -> (f64, f64) {
    let b = out.far.as_ref().unwrap();
    let (nt, np) = (60, 90);
    let (mut total, mut peak) = (0.0, 0.0f64);
    for i in 0..nt {
        let th = (i as f64 + 0.5) / nt as f64 * PI;
        for j in 0..np {
            let ph = (j as f64 + 0.5) / np as f64 * 2.0 * PI;
            let u = ntff::intensity(&b.far([th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()]));
            total += u * th.sin() * (PI / nt as f64) * (2.0 * PI / np as f64);
            peak = peak.max(u);
        }
    }
    (total, peak)
}

fn resonance(out: &Outcome) -> f64 {
    let pts: Vec<_> = out.sweep.freqs.iter().zip(&out.sweep.z).collect();
    pts.windows(2).find(|w| w[0].1.im < 0.0 && w[1].1.im >= 0.0).map(|w| *w[0].0).unwrap_or(0.0)
}

#[test]
fn a_dipole_conserves_power_and_has_dipole_directivity() {
    let out = run(&dipole(), &Options::default(), |_, _| true).unwrap();
    let (total, peak) = radiated(&out);
    let eff = total / out.p_in;
    let d = 10.0 * (4.0 * PI * peak / total).log10();
    assert!((eff - 1.0).abs() < 0.01, "power balance {eff}");
    assert!((d - 2.15).abs() < 0.05, "directivity {d}");
    let f = resonance(&out);
    assert!((960e6..1010e6).contains(&f), "resonance {f}");
}

#[test]
fn the_gpu_run_agrees_with_the_cpu_run() {
    if gpu::block_on(gpu::adapter()).is_none() {
        eprintln!("no GPU adapter; nothing to compare");
        return;
    }
    let cpu = run(&dipole(), &Options::default(), |_, _| true).unwrap();
    let gpu = gpu::block_on(run_gpu(&dipole(), &Options::default(), |_, _| true)).unwrap();
    for (a, b) in cpu.sweep.z.iter().zip(&gpu.sweep.z).step_by(10) {
        assert!((a - b).norm() < 0.01 * a.norm() + 0.5, "{a} vs {b}");
    }
    let (tc, pc) = radiated(&cpu);
    let (tg, pg) = radiated(&gpu);
    assert!((tc / tg - 1.0).abs() < 0.01 && (pc / pg - 1.0).abs() < 0.01);
}

fn quad(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> [Triangle; 2] {
    [[a, b, c], [a, c, d]]
}

fn plate(lo: [f64; 2], hi: [f64; 2], z: f64) -> Vec<Triangle> {
    quad([lo[0], lo[1], z], [hi[0], lo[1], z], [hi[0], hi[1], z], [lo[0], hi[1], z]).to_vec()
}

fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Vec<Triangle> {
    let p = |i: usize| [0, 1, 2].map(|a| if i >> a & 1 == 1 { hi[a] } else { lo[a] });
    let faces =
        [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
    faces.iter().flat_map(|f| quad(p(f[0]), p(f[1]), p(f[2]), p(f[3]))).collect()
}

fn sheen(meshed: bool) -> Model {
    let mm = 1e-3;
    let h = 0.794 * mm;
    let (w, l) = (12.45 * mm, 16.0 * mm);
    let (lw, loff) = (2.46 * mm, 2.09 * mm);
    let (x0, y0) = (-w / 2.0, -l / 2.0);
    let line_x = x0 + loff;
    let edge_y = y0 - 8.0 * mm;
    let gx = 30.0 * mm;
    let top = l / 2.0 + 7.0 * mm;
    let sheets = [
        ([-gx / 2.0, edge_y], [gx / 2.0, top], 0.0),
        ([x0, y0], [x0 + w, y0 + l], h),
        ([line_x, edge_y], [line_x + lw, y0], h),
    ];
    let board = ([-gx / 2.0, edge_y, 0.0], [gx / 2.0, top, h]);
    let mut model = Model {
        parts: Vec::new(),
        metal: Vec::new(),
        dielectrics: Vec::new(),
        port: Some(Port {
            a: [line_x + lw / 2.0, edge_y, 0.0],
            b: [line_x + lw / 2.0, edge_y, h],
            r: 50.0,
        }),
        f0: 7.5e9,
        span: 0.25,
        fine: 0.4 * mm,
        ground_plane_z: None,
    };
    if meshed {
        for (lo, hi, z) in sheets {
            model.parts.push(Part { triangles: plate(lo, hi, z), stuff: Stuff::Metal });
        }
        model.parts.push(Part {
            triangles: cuboid(board.0, board.1),
            stuff: Stuff::Dielectric { eps_r: 2.2, tan_d: 0.0 },
        });
    } else {
        for (lo, hi, z) in sheets {
            model.metal.push(Aabb::new([lo[0], lo[1], z], [hi[0], hi[1], z]));
        }
        model.dielectrics.push(Dielectric {
            bounds: Aabb::new(board.0, board.1),
            eps_r: 2.2,
            tan_d: 0.0,
        });
    }
    model
}

fn best_match(model: &Model) -> (f64, f64) {
    let opts = Options { far_field: false, ..Options::default() };
    let out = match gpu::block_on(gpu::adapter()) {
        Some(_) => gpu::block_on(run_gpu(model, &opts, |_, _| true)).unwrap(),
        None => run(model, &opts, |_, _| true).unwrap(),
    };
    let (f, z) = out
        .sweep
        .freqs
        .iter()
        .zip(&out.sweep.z)
        .min_by(|a, b| {
            ((a.1 - 50.0) / (a.1 + 50.0)).norm().total_cmp(&((b.1 - 50.0) / (b.1 + 50.0)).norm())
        })
        .unwrap();
    (*f, 20.0 * ((z - 50.0) / (z + 50.0)).norm().log10())
}

#[test]
fn sheens_microstrip_patch_resonates_where_it_was_measured() {
    for meshed in [false, true] {
        let (f, s11) = best_match(&sheen(meshed));
        eprintln!("meshed {meshed}: S11 {s11:.1} dB at {:.4} GHz", f / 1e9);
        assert!((f / 7.5e9 - 1.0).abs() < 0.015, "meshed {meshed}: S11 minimum at {f}");
        assert!(s11 < -10.0, "meshed {meshed}: {s11} dB");
    }
}

#[test]
fn an_open_dielectric_mesh_is_refused() {
    let mut model = sheen(true);
    model.parts.push(Part {
        triangles: plate([0.0, 0.0], [1e-3, 1e-3], 2e-3),
        stuff: Stuff::Dielectric { eps_r: 3.0, tan_d: 0.0 },
    });
    let opts = Options { far_field: false, max_steps: 10, ..Options::default() };
    let err = run(&model, &opts, |_, _| true).err();
    assert_eq!(err.as_deref(), Some("a dielectric part is not a closed mesh"));
}

#[test]
fn a_volume_result_reports_consistent_gains() {
    let out = run(&dipole(), &Options::default(), |_, _| true).unwrap();
    let r = antenna_solver::solve::volume_result(&out);
    let p = r.pattern.as_ref().unwrap();
    let broadside = r.gain_dbi([1.0, 0.0, 0.0]).unwrap();
    let dbi = r.dbi.unwrap();
    eprintln!("dbi {dbi} broadside {broadside} peak {} p {}", r.peak, p([1.0, 0.0, 0.0]));
    assert!((broadside - dbi).abs() < 0.2, "{broadside} vs {dbi}");
    assert!(r.gain_dbi([0.0, 0.0, 1.0]).unwrap() < dbi - 20.0);
}
