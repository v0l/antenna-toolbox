use crate::bands::{HF_BANDS, bands_from, lowest_band};
use crate::draw::{Anchor, GOLD, GREEN, SILVER};
use crate::feed_detail::FeedDetail;
use crate::sketch::Sketch;
use crate::{Build, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::units::C;

pub static RANDOM: Design = Design {
    id: "random",
    name: "Random wire on a 9:1 unun",
    group: Group::TwoSide,
    build: Build::Wire,
    gain: "2.2 dBi",
    controls: &[],
    polarisation: "**Linear, along the wire.** On the lowest band the wire is shorter than a \
                   half wave and gives a dipole's figure-eight. On the higher bands it is \
                   several half waves long and the lobes swing round toward the far end.",
    compute,
};

const LENGTHS: [(f64, f64); 10] = [
    (0.340, 0.200),
    (0.350, 0.200),
    (0.360, 0.200),
    (0.290, 0.200),
    (0.295, 0.175),
    (0.295, 0.200),
    (0.275, 0.200),
    (0.310, 0.175),
    (0.355, 0.125),
    (0.415, 0.075),
];

fn default_lengths(c: &Ctx) -> (f64, f64) {
    let freq = C / c.lam;
    match lowest_band(freq) {
        Some(i) => {
            let lam = C / HF_BANDS[i].centre();
            (LENGTHS[i].0 * lam, LENGTHS[i].1 * lam)
        }
        None => (LENGTHS[9].0 * c.lam, LENGTHS[9].1 * c.lam),
    }
}

fn band_span(freq: f64) -> String {
    match bands_from(freq) {
        [] => "no amateur HF band at or above this frequency".into(),
        [one] => format!("{} only", one.name),
        [first, .., last] => format!("{} to {}, with a tuner", first.name, last.name),
    }
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let (len0, cp0) = default_lengths(c);
    let len = c.P("radiator", len0);
    let cp = c.P("counterpoise", cp0);
    let g = (lam / 400.0).min(50.0);
    Output {
        spec: "~2 dBi on the lowest band · any length that avoids a half wave · 9:1 unun, then a \
               tuner"
            .into(),
        rows: vec![
            row("**radiator**, from the unun to the far end", c.fmt(len)),
            row("**counterpoise**, from the cold end of the unun", c.fmt(cp)),
            row("9:1 unun", "9 turns of three wires side by side on an FT-240-43"),
            row("bands", band_span(C / lam)),
            total("total wire", c.fmt(len + cp)),
        ],
        scene: Scene {
            wires: vec![
                wire(vec![[g, 0.0, 0.0], [g + len, 0.0, 0.0]], GOLD, 3.5),
                wire(vec![[-g, 0.0, 0.0], [-g - cp, 0.0, 0.0]], SILVER, 2.5),
            ],
            feed: Some([0.0; 3]),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([(len - cp) / 2.0, 0.0, 0.0]),
            pol: "polarisation: linear, along the wire".into(),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(
            vec![vec![[-g - cp, 0.0, 0.0], [-g, 0.0, 0.0], [g, 0.0, 0.0], [g + len, 0.0, 0.0]]],
            [0.0; 3],
        )),
        diagram: Sketch::new()
            .wire(&[(g, 0.0), (g + len, 0.0)], GOLD, 3.0)
            .wire(&[(-g - cp, 0.0), (-g, 0.0)], SILVER, 2.5)
            .dim((g, 0.06 * len), (g + len, 0.06 * len), "radiator", (0.0, -8.0), Anchor::Middle)
            .dim(
                (-g - cp, 0.06 * len),
                (-g, 0.06 * len),
                "counterpoise",
                (0.0, -8.0),
                Anchor::Middle,
            )
            .note((0.0, -0.1 * len), "9:1 unun", GREEN, Anchor::Middle)
            .caption("broadside on the lowest band, toward the far end higher up", GREEN)
            .render(),
        feed: FeedDetail::Unun,
        notes: "**One wire for every band, with a tuner doing the matching.** A random wire is \
                deliberately not resonant anywhere. Fed at one end, a wire that happens to be \
                a whole number of half waves long shows several kΩ, which no tuner reaches. \
                Away from those lengths the end impedance sits between a few tens and a few \
                hundred ohms with reactance on top, and a 9:1 unun brings that down to where \
                a tuner can finish the job.\n\n\
                **The length here is solved, not taken from a chart.** The radiator and \
                counterpoise were chosen by solving this wire on every band from the lowest \
                one at or above your frequency up to 10 m, band edges included, and keeping the \
                pair whose worst SWR after the unun was lowest, with 3% either way still \
                holding. On most bands that leaves 3:1 to 7:1 at the radio: an external or \
                automatic tuner copes, a radio's built-in tuner usually stops at 3:1. The \
                bands table shows the solved SWR on each band.\n\n\
                **The counterpoise is half the antenna.** The unun needs something to push \
                against. Run the counterpoise out from the cold end, along the ground or a \
                fence; more than one wire does no harm. Without one, the coax braid takes the \
                return current, the radio gets RF in the shack and the SWR moves when you \
                touch the cable, so keep a choke on the coax at the unun either way.\n\n\
                The model is in free space. Height and the ground under a real install move \
                the numbers, which is why the choice holds a margin rather than sitting on a \
                sharp dip."
            .into(),
        cut: None,
    }
}
