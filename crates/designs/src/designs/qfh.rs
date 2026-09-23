use crate::draw::{Anchor, GOLD, GREEN, SILVER};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::geometry::{Geometry, WireGeometry};
use antenna_solver::vec::Vec3;
use std::f64::consts::PI;

pub static QFH: Design = Design {
    id: "qfh",
    name: "Quadrifilar helix",
    group: Group::Omni,
    build: Build::Wire,
    gain: "4.9 dBi",
    controls: &[ControlId::Hand],
    polarisation: "**Circular, over almost the whole sky.** Two twisted loops of slightly \
                   different size, fed together at the top, phase themselves a quarter cycle \
                   apart. The result hears a satellite from the zenith down to near the \
                   horizon with nearly constant signal, which is why it is the standard 137 MHz \
                   weather satellite antenna.",
    compute,
};

struct Loop {
    r: f64,
    h: f64,
    phi: f64,
}

fn helix(l: &Loop, from: f64, sense: f64, steps: usize) -> Vec<Vec3> {
    (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let a = from + sense * PI * t;
            [l.r * a.cos(), l.r * a.sin(), -l.h * t]
        })
        .collect()
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let sense = if c.ctl(ControlId::Hand) == 0.0 { 1.0 } else { -1.0 };
    let big = Loop {
        r: c.P("large diameter", 0.1761 * lam) / 2.0,
        h: c.P("large height", 0.2553 * lam),
        phi: 0.0,
    };
    let small = Loop {
        r: c.P("small diameter", 0.1649 * lam) / 2.0,
        h: c.P("small height", 0.2391 * lam),
        phi: -sense * PI / 2.0,
    };
    let g = 0.003 * lam;
    let (hot, cold) = ([0.0, 0.0, g], [0.0, 0.0, -g]);
    let mut lines: Vec<Vec<Vec3>> = vec![vec![cold, hot]];
    let mut wires = Vec::new();
    for (l, colour) in [(&big, GOLD), (&small, SILVER)] {
        let at = |a: f64, z: f64| [l.r * a.cos(), l.r * a.sin(), z];
        let (a0, a1) = (l.phi, l.phi + PI);
        let legs = [helix(l, a0, sense, 24), helix(l, a1, sense, 24)];
        lines.push(vec![at(a0, 0.0), hot]);
        lines.push(vec![at(a1, 0.0), cold]);
        lines.push(legs[0].clone());
        lines.push(legs[1].clone());
        lines.push(vec![*legs[0].last().unwrap(), *legs[1].last().unwrap()]);
        wires.push(wire(vec![at(a0, 0.0), [0.0; 3], at(a1, 0.0)], colour, 3.0));
        wires.push(wire(legs[0].clone(), colour, 3.0));
        wires.push(wire(legs[1].clone(), colour, 3.0));
        wires.push(wire(vec![*legs[0].last().unwrap(), *legs[1].last().unwrap()], colour, 3.0));
    }
    let helix_len = |l: &Loop| (l.h * l.h + (PI * l.r).powi(2)).sqrt();
    let per = |l: &Loop| 4.0 * l.r + 2.0 * helix_len(l);
    Output {
        spec: "~4 dBi · circular over the whole sky · about 50 Ω".into(),
        rows: vec![
            row("**large loop** diameter", c.fmt(2.0 * big.r)),
            row("large loop height", c.fmt(big.h)),
            row("large loop, each helical leg", c.fmt(helix_len(&big))),
            row("**small loop** diameter", c.fmt(2.0 * small.r)),
            row("small loop height", c.fmt(small.h)),
            row("small loop, each helical leg", c.fmt(helix_len(&small))),
            row("twist", "half a turn, both loops"),
            total("total wire", c.fmt(per(&big) + per(&small))),
        ],
        scene: Scene {
            wires,
            feed: Some([0.0; 3]),
            omni: true,
            omni_y: Some(-big.h / 2.0),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pattern_origin: Some([0.0, 0.0, -big.h / 2.0]),
            pol: "polarisation: circular".into(),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(WireGeometry::new(lines, [0.0; 3])),
        diagram: Sketch::new()
            .wire(
                &(0..=24)
                    .map(|i| {
                        let t = i as f64 / 24.0;
                        (big.r * (sense * PI * t).cos(), -big.h * t)
                    })
                    .collect::<Vec<_>>(),
                GOLD,
                3.0,
            )
            .wire(
                &(0..=24)
                    .map(|i| {
                        let t = i as f64 / 24.0;
                        (big.r * (PI + sense * PI * t).cos(), -big.h * t)
                    })
                    .collect::<Vec<_>>(),
                GOLD,
                3.0,
            )
            .wire(&[(-big.r, 0.0), (big.r, 0.0)], GOLD, 3.0)
            .wire(&[(-big.r, -big.h), (big.r, -big.h)], GOLD, 3.0)
            .wire(&[(-small.r, -small.h), (small.r, -small.h)], SILVER, 2.5)
            .dim(
                (-big.r * 1.6, 0.0),
                (-big.r * 1.6, -big.h),
                "large height",
                (-8.0, 4.0),
                Anchor::End,
            )
            .dim(
                (big.r * 1.6, 0.0),
                (big.r * 1.6, -small.h),
                "small height",
                (8.0, 4.0),
                Anchor::Start,
            )
            .note(
                (0.0, 0.03 * lam),
                "feed at the top, both loops in parallel",
                GREEN,
                Anchor::Middle,
            )
            .caption("large loop drawn, small loop turned 90° inside it", SILVER)
            .tall(360.0)
            .render(),
        feed: inline(
            "one half of each loop's top wire",
            "the other half of each loop's top wire",
            Some("Run the coax down the centre of the mast, with a few ferrites where it leaves."),
        ),
        notes: "**Two bifilar loops, each half a turn of twist, one inside the other at right \
                angles.** The large loop is a few percent longer than resonance and the small \
                one a few percent shorter, so their currents land 90° apart when both are fed \
                in parallel at the top. That self-phasing is what makes the circular \
                polarisation; no hybrid or delay line needed.\n\n**Dimensions follow the \
                classic Kilgus design** (height to diameter 1.45, loops 1.105 and 1.035 λ), the \
                one behind most home-built NOAA and Meteor antennas. The hand of the twist sets \
                the hand of the polarisation: weather satellites transmit right-hand \
                circular.\n\nBuilt from copper tube, the bends at the corners use up length, \
                so real builds cut each loop a little longer. The coax is fed up the central \
                mast, which is also the reason a QFH barely needs a balun."
            .into(),
        cut: None,
    }
}
