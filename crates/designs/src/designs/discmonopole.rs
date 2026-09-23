use crate::draw::{Anchor, Drawing, GOLD, GREY, SILVER, Stroke, hexa};
use crate::feed_detail::vertical;
use crate::{Build, ControlId, CutFile, Ctx, Design, Group, Output, Scene, row, total};
use antenna_solver::geometry::{Geometry, Mesh, SurfaceGeometry};
use antenna_solver::surface::mesh::{merge_meshes, mesh_profile};

pub static DISCMONOPOLE: Design = Design {
    id: "discmono",
    name: "Disc monopole",
    group: Group::Omni,
    build: Build::Sheet,
    gain: "2 dBi",
    controls: &[ControlId::FeedGap],
    polarisation: "**Vertical, near omnidirectional in azimuth.** A monopole pattern: a doughnut \
                   around the vertical axis with a deep null straight up along it. The near is \
                   doing work in that sentence, because the sheet is flat rather than a body of \
                   revolution, so the azimuth circle is not a perfect one. Expect a decibel or two \
                   of ripple round the horizon, which is the price of cutting a body of revolution \
                   out of flat stock.",
    compute,
};

#[derive(Clone, Copy)]
struct Shape {
    r: f64,
    gap: f64,
    neck: f64,
    gp_w: f64,
    gp_h: f64,
    taper: f64,
}

struct Heights {
    gp_top: f64,
    feed: f64,
    disc_bottom: f64,
    disc_centre: f64,
    top: f64,
}

impl Shape {
    fn heights(&self) -> Heights {
        Heights {
            gp_top: self.gp_h,
            feed: self.gp_h + self.gap / 2.0,
            disc_bottom: self.gp_h + self.gap,
            disc_centre: self.gp_h + self.gap + self.r,
            top: self.gp_h + self.gap + 2.0 * self.r,
        }
    }

    fn half_width(&self, y: f64) -> f64 {
        let h = self.heights();
        if y <= h.gp_top - self.taper {
            return self.gp_w / 2.0;
        }
        if y <= h.gp_top {
            let t = (h.gp_top - y) / self.taper;
            return self.neck / 2.0 + t * (self.gp_w / 2.0 - self.neck / 2.0);
        }
        if y <= h.disc_bottom {
            return self.neck / 2.0;
        }
        let dy = y - h.disc_centre;
        (self.neck / 2.0).max((self.r * self.r - dy * dy).max(0.0).sqrt())
    }

    fn mesh(&self, cell: f64, across: usize) -> Mesh {
        let h = self.heights();
        let section = |y0: f64, y1: f64, stations: usize| {
            let m = mesh_profile(y0, y1, stations, |y| self.half_width(y), 0.0, across);
            Mesh { vertices: m.vertices.iter().map(|v| [v[1], v[0], v[2]]).collect(), triangles: m.triangles }
        };
        let count = |span: f64, target: f64| ((span / target).round() as usize).max(2);
        merge_meshes(
            &[
                section(0.0, h.gp_top - self.taper, count(self.gp_h - self.taper, cell)),
                section(h.gp_top - self.taper, h.gp_top, count(self.taper, cell / 2.0)),
                section(h.gp_top, h.feed, 2),
                section(h.feed, h.disc_bottom, 2),
                section(h.disc_bottom, h.top, count(2.0 * self.r, cell)),
            ],
            self.gap.min(self.neck) / 50.0,
        )
    }
}

fn cut_of(s: &Shape, fmt: &dyn Fn(f64) -> String) -> CutFile {
    let h = s.heights();
    let hw = s.gp_w / 2.0;
    let hn = s.neck / 2.0;
    CutFile {
        loops: vec![vec![
            [-hw, 0.0, 0.0],
            [hw, 0.0, 0.0],
            [hw, h.gp_top - s.taper, 0.0],
            [hn, h.gp_top, 0.0],
            [-hn, h.gp_top, 0.0],
            [-hw, h.gp_top - s.taper, 0.0],
        ]],
        circles: vec![([0.0, h.disc_centre], s.r)],
        note: format!(
            "Two pieces: a {} disc and a {} by {} ground plane with the shoulder narrowing to {} \
             at the top. They are drawn at their working spacing, {} apart, which the connector \
             bridges: pin to the disc, shield to the ground.",
            fmt(2.0 * s.r),
            fmt(s.gp_w),
            fmt(s.gp_h),
            fmt(s.neck),
            fmt(s.gap)
        ),
    }
}

fn diagram(s: &Shape) -> Drawing {
    let h = s.heights();
    let w = 640.0;
    let pad = 54.0;
    let widest = s.gp_w.max(2.0 * s.r);
    let sc = (300.0 / widest).min(300.0 / h.top);
    let height = h.top * sc + 2.0 * pad;
    let cx = 250.0;
    let y = |v: f64| pad + (h.top - v) * sc;
    let x = |v: f64| cx + v * sc;
    let gw = s.gp_w / 2.0 * sc;
    let nw = (s.neck / 2.0 * sc).max(2.0);
    let mut g = Drawing::new(w, height);
    g.circle(cx, y(h.disc_centre), s.r * sc, Some(hexa(0xe8b23a, 0.18)), Some(Stroke::new(GOLD, 2.5)));
    g.polygon(
        &[
            (cx - gw, y(0.0)),
            (cx + gw, y(0.0)),
            (cx + gw, y(h.gp_top - s.taper)),
            (cx + nw, y(h.gp_top)),
            (cx - nw, y(h.gp_top)),
            (cx - gw, y(h.gp_top - s.taper)),
        ],
        Some(hexa(0xc9d1d6, 0.14)),
        Some(Stroke::new(SILVER, 2.5)),
    );
    g.rect(cx - nw, y(h.disc_bottom), 2.0 * nw, (s.gap * sc).max(2.0), Some(hexa(0xe8b23a, 0.35)), Some(Stroke::new(GOLD, 1.0)));
    g.feed_flag(cx, y(h.feed), false);
    g.dim(cx, y(h.disc_centre), x(s.r), y(h.disc_centre), "r", 0.0, -6.0, Anchor::Middle);
    g.dim(x(s.gp_w / 2.0) + 24.0, y(0.0), x(s.gp_w / 2.0) + 24.0, y(h.gp_top), "ground height", 28.0, 4.0, Anchor::Start);
    g.dim(cx - gw, y(0.0) + 24.0, cx + gw, y(0.0) + 24.0, "ground width", 0.0, -6.0, Anchor::Middle);
    g.dim(cx - nw - 30.0, y(h.gp_top), cx - nw - 30.0, y(h.disc_bottom), "gap", -8.0, 4.0, Anchor::End);
    g.label(x(s.gp_w / 2.0) + 8.0, y(h.gp_top - s.taper / 2.0), "shoulder", GREY, 11.0, Anchor::Start);
    g.beam_label(cx, height - 12.0, Some("equal in all directions around the vertical axis"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let r = c.P("disc radius", 0.085 * lam);
    let gap_pct = c.ctl(ControlId::FeedGap);
    let s = Shape {
        r,
        gap: gap_pct / 100.0 * r,
        neck: c.P("feed tab width", 0.03 * lam),
        gp_w: c.P("ground width", 0.45 * lam),
        gp_h: c.P("ground height", 0.25 * lam),
        taper: c.P("shoulder height", lam / 25.0),
    };
    let mesh = s.mesh(lam / 25.0, 6);
    let h = s.heights();
    Output {
        spec: "broadband · sheet metal · solved with RWG".into(),
        rows: vec![
            row("**disc radius**", c.fmt(r)),
            row("disc diameter, cut this circle", c.fmt(2.0 * r)),
            row(format!("**feed gap**, {gap_pct:.0}% of the radius"), c.fmt(s.gap)),
            row("feed tab width", c.fmt(s.neck)),
            row("**ground** width", c.fmt(s.gp_w)),
            row("**ground** height", c.fmt(s.gp_h)),
            row("shoulder height into the tab", c.fmt(s.taper)),
            row("overall height", c.fmt(h.top)),
            total("triangles in the model", mesh.triangles.len().to_string()),
        ],
        scene: Scene {
            mesh: Some(mesh.clone()),
            feed: Some([0.0, h.feed, 0.0]),
            omni: true,
            omni_y: Some(h.feed),
            pattern_origin: Some([0.0, h.feed, 0.0]),
            pol: "polarisation: vertical, near omnidirectional in azimuth".into(),
            ..Default::default()
        },
        solve: Geometry::Surface(SurfaceGeometry {
            mesh,
            feed: [0.0, h.feed, 0.0],
            feed_dir: [0.0, 1.0, 0.0],
            feed_tol: s.gap / 20.0,
        }),
        diagram: diagram(&s),
        feed: vertical(
            "disc, on the centre pin",
            "ground plane, on the shield",
            Some(
                "Cut the disc and the ground plane as two separate pieces and let the connector \
                 bridge the gap. The strip drawn across the gap is what the solver uses in place \
                 of the pin.",
            ),
        ),
        notes: "**Two shapes and a connector, and it covers a band a dipole could not.** A \
                circular disc standing over a rectangular ground plane, both cut from the same \
                sheet, fed across the small gap between them. Nothing is resonant in the usual \
                sense: the disc is fat enough that current finds a path of the right length across \
                a wide range of frequencies, which is why this shape turns up in every UWB radio \
                and every cheap wideband scanner antenna. Cut at the default proportions it solves \
                close to 50 Ω with little reactance, so it is one of the very few antennas here \
                that wants no matching of any kind.\n\n**The ground plane is half the antenna, so \
                do not trim it.** This is the coarse control, not the gap. Shrinking the ground \
                drops the impedance and adds capacitive reactance; growing it pushes the other way. \
                The sloped shoulder where the ground narrows into the feed tab earns its place too: \
                flatten it and the match drifts.\n\n**The gap is the fine trim.** Walking it from \
                2% to 20% of the disc radius moves the solved impedance up by about ten ohms, \
                smoothly and with no surprises, so it is the knob to reach for once the ground \
                plane is settled. Published designs are far twitchier about this dimension than the \
                model is, because they feed the disc with a bare connector pin where this model \
                uses a tab a few millimetres wide. A wide tab swamps the capacitance of the gap.\n\n\
                **Broad, but not the decade the textbooks quote.** The decade figures belong to the \
                printed version, where the ground plane sits on the back of a substrate and couples \
                to the disc through it. Cut from one flat sheet the ground has to sit beside the \
                disc instead, and that costs bandwidth at the bottom."
            .into(),
        cut: Some(cut_of(&s, c.fmt)),
    }
}
