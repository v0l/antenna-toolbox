use crate::draw::{Anchor, Drawing, GOLD, GREY, Stroke, hexa};
use crate::export::{flat, sample_curve};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, CutFile, Design, Group, Output, Scene, row, total};
use antenna_solver::geometry::{Geometry, Mesh, SurfaceGeometry};
use antenna_solver::surface::mesh::{merge_meshes, mesh_strip};

pub static VIVALDI: Design = Design {
    id: "vivaldi",
    name: "Vivaldi",
    group: Group::Beam,
    build: Build::Sheet,
    gain: "7 dBi",
    controls: &[ControlId::Mouth],
    polarisation: "**Linear, across the slot.** The field lives in the gap, so the polarisation \
                   runs perpendicular to the slot and stays in the plane of the sheet. That catches \
                   people out: hold the sheet flat with the slot running left to right and you get \
                   vertical polarisation, which is the opposite of what the outline suggests. The \
                   beam is endfire, straight out of the mouth, and both faces of the sheet radiate \
                   alike.",
    compute,
};

struct Shape {
    x_back: f64,
    flare_len: f64,
    band: f64,
    bridge_len: f64,
    w0: f64,
    feed_x: f64,
    rate: f64,
    stations: usize,
    across: usize,
    cell_x: f64,
}

impl Shape {
    fn new(
        flare_len: f64,
        throat: f64,
        w0: f64,
        mouth: f64,
        band: f64,
        bridge: f64,
        cell: f64,
    ) -> Self {
        let x_back = -throat;
        let total = flare_len - x_back;
        let stations = ((total / cell).round() as usize).max(8);
        let across = ((band / cell).round() as usize).max(2);
        let cell_x = total / stations as f64;
        let bridge_cells = ((bridge / cell_x).round() as usize).max(2);
        let bridge_len = cell_x * bridge_cells as f64;
        let rate = (mouth / w0).ln() / flare_len;
        Shape {
            x_back,
            flare_len,
            band,
            bridge_len,
            w0,
            feed_x: x_back + bridge_len / 2.0,
            rate,
            stations,
            across,
            cell_x,
        }
    }

    fn slot_half(&self, x: f64) -> f64 {
        if x <= self.x_back + self.bridge_len + self.cell_x * 1e-6 {
            0.0
        } else if x <= 0.0 {
            self.w0 / 2.0
        } else {
            self.w0 / 2.0 * (self.rate * x).exp()
        }
    }

    fn mesh(&self) -> Mesh {
        let upper = mesh_strip(
            self.x_back,
            self.flare_len,
            self.stations,
            |x| self.slot_half(x),
            |x| self.slot_half(x) + self.band,
            0.0,
            self.across,
        );
        let lower = mesh_strip(
            self.x_back,
            self.flare_len,
            self.stations,
            |x| -(self.slot_half(x) + self.band),
            |x| -self.slot_half(x),
            0.0,
            self.across,
        );
        let tol = (self.flare_len - self.x_back) / self.stations as f64 / 100.0;
        merge_meshes(&[upper, lower], tol)
    }
}

fn cut_of(s: &Shape, mouth: f64, fmt: &dyn Fn(f64) -> String) -> CutFile {
    let slot_end = s.x_back + s.bridge_len;
    let edge = s.w0 / 2.0 + s.band;
    let half = |sign: f64| {
        let mut pts = vec![[s.x_back, sign * edge]];
        pts.extend(sample_curve(|x| [x, sign * (s.slot_half(x) + s.band)], 0.0, s.flare_len, 1.0));
        pts.extend(sample_curve(|x| [x, sign * s.slot_half(x)], s.flare_len, 0.0, 0.5));
        pts.extend([[slot_end, sign * (s.w0 / 2.0)], [slot_end, 0.0], [s.x_back, 0.0]]);
        flat(&pts)
    };
    CutFile {
        loops: vec![half(1.0), half(-1.0)],
        circles: vec![],
        note: format!(
            "Two mirror image halves, together {} by {}, drawn in place. They meet along the {} \
             seam at the closed end, and that seam is the feed: pin to one half, shield to the \
             other. Do not join them there. The slot is {} through the throat and opens \
             exponentially to {} at the mouth; the outer edge follows the flare because metal \
             beyond it does nothing.",
            fmt(s.flare_len - s.x_back),
            fmt(mouth + 2.0 * s.band),
            fmt(s.bridge_len),
            fmt(s.w0),
            fmt(mouth)
        ),
    }
}

fn diagram(s: &Shape, mouth: f64) -> Drawing {
    let w = 640.0;
    let pad = 46.0;
    let span = s.flare_len - s.x_back;
    let sc = (w - 2.0 * pad) / span;
    let half_max = (mouth / 2.0 + s.band) * sc;
    let h = 2.0 * half_max + 2.0 * pad;
    let cy = h / 2.0;
    let px = |x: f64| pad + (x - s.x_back) * sc;
    let py = |y: f64| cy - y * sc;
    let xs: Vec<f64> = (0..=80).map(|i| s.x_back + span * i as f64 / 80.0).collect();
    let mut g = Drawing::new(w, h);
    for sign in [1.0, -1.0] {
        let mut pts: Vec<(f64, f64)> =
            xs.iter().map(|&x| (px(x), py(sign * (s.slot_half(x) + s.band)))).collect();
        pts.extend(xs.iter().rev().map(|&x| (px(x), py(sign * s.slot_half(x)))));
        g.polygon(&pts, Some(hexa(0xe8b23a, 0.18)), Some(Stroke::new(GOLD, 2.5)));
    }
    let xf = px(s.feed_x);
    g.dot(xf, py(0.0) - 4.0, 3.0, GOLD);
    g.dot(xf, py(0.0) + 4.0, 3.0, GREY);
    g.feed_flag(xf, py(0.0), true);
    let top = py(mouth / 2.0 + s.band) - 22.0;
    g.dim(px(0.0), top, px(s.flare_len), top, "flare length", 0.0, -6.0, Anchor::Middle);
    let xm = px(s.flare_len) + 22.0;
    g.dim(xm, py(mouth / 2.0), xm, py(-mouth / 2.0), "mouth", 30.0, 5.0, Anchor::Middle);
    g.label(px(s.x_back), py(0.0) - 14.0, "closed end", GREY, 12.0, Anchor::Start);
    g.beam_label(w / 2.0, h - 12.0, Some("beam fires endfire, out of the mouth to the right"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let flare_len = c.P("flare length", 1.2 * lam);
    let throat = c.P("throat", 0.18 * lam);
    let w0 = c.P("slot width", lam / 40.0);
    let band = c.P("metal band", 0.4 * lam);
    let bridge = c.P("feed bridge", 0.15 * lam);
    let mouth = c.ctl(ControlId::Mouth) * lam;
    let shape = Shape::new(flare_len, throat, w0, mouth, band, bridge, lam / 14.0);
    let mesh = shape.mesh();
    Output {
        spec: "endfire · wideband · sheet metal · solved with RWG".into(),
        rows: vec![
            row("**flare length**, throat to mouth", c.fmt(flare_len)),
            row("**mouth** opening", c.fmt(mouth)),
            row("**slot width** at the throat", c.fmt(w0)),
            row("**throat**, parallel run behind the flare", c.fmt(throat)),
            row("**metal band** either side of the slot", c.fmt(band)),
            row("**feed bridge** at the closed end", c.fmt(shape.bridge_len)),
            row("flare rate, e-folds per wavelength", format!("{:.2}", shape.rate * lam)),
            row(
                "sheet overall, length by width",
                format!("{} by {}", c.fmt(flare_len + throat), c.fmt(mouth + 2.0 * band)),
            ),
            total("triangles in the model", mesh.triangles.len().to_string()),
        ],
        scene: Scene {
            mesh: Some(mesh.clone()),
            feed: Some([shape.feed_x, 0.0, 0.0]),
            beam_vec: Some([1.0, 0.0, 0.0]),
            pattern_origin: Some([(flare_len - throat) / 2.0, 0.0, 0.0]),
            pol: "polarisation: linear, across the slot".into(),
            ..Default::default()
        },
        solve: Geometry::Surface(SurfaceGeometry {
            mesh,
            feed: [shape.feed_x, 0.0, 0.0],
            feed_dir: [0.0, 1.0, 0.0],
            feed_tol: w0 / 6.0,
        }),
        diagram: diagram(&shape, mouth),
        feed: inline(
            "upper half, on the centre pin",
            "lower half, on the shield",
            Some("Feed across the slot at the closed end, where the two halves join."),
        ),
        notes: "**The one sheet antenna here with real forward gain.** Cut an exponential slot in \
                a piece of copper clad and a wave will ride along the gap, stay bound while the \
                gap is narrow, and let go once the gap approaches half a wavelength. That gradual \
                release is what makes it endfire, and it is also what makes it wideband: there is \
                no resonant length anywhere in it, only a taper that runs out of grip at a \
                different place for every frequency. The solver gives about 7 dBi for the default \
                size with 14 dB of front to back, which is what a tapered slot of a bit over a \
                wavelength should do.\n\n**Length and mouth have to grow together.** Making it \
                longer on its own does almost nothing, because with the mouth fixed you have only \
                made the taper gentler. Grow both and the gain follows: 1 λ long with a 0.45 λ \
                mouth solves to 7.1 dBi, 1.5 by 0.65 gives 7.9, and 2 by 0.85 gives 8.5. The slot \
                width at the throat sets the top of the band and is the dimension worth cutting \
                carefully. The metal band either side of the slot matters more than people expect, \
                and narrowing it below about a third of a wavelength costs better than a decibel.\n\n\
                **The impedance shown is the slot, not what your SMA will see.** A real Vivaldi is \
                built on double sided board and fed through a slotline to microstrip transition: a \
                microstrip line on the back crosses the slot and ends in a stub, and that stub is \
                what turns the slot impedance into something a 50 Ω cable will accept. There is no \
                transition in this model. It puts a voltage straight across the slot at the closed \
                end, spread over the feed bridge, so the number in the corner is the raw slot \
                impedance, and that sits in the low hundreds rather than at 50. Set the SWR \
                reference to 100 Ω to see the wideband behaviour the design is famous for. Build it \
                with a proper transition, or feed it through a 1:3 transformer, or just accept the \
                mismatch on receive where a few decibels against 7 dBi of gain is still a clear \
                win.\n\n**Cutting it.** The slot is the only thing you have to cut accurately. Metal \
                far from the slot carries very little current, which is why the outline here \
                follows the flare instead of being a rectangle: it performs the same and uses a \
                good deal less sheet. Keep the curve smooth, because a stepped or nibbled edge \
                scatters and fills in the pattern."
            .into(),
        cut: Some(cut_of(&shape, mouth, c.fmt)),
    }
}
