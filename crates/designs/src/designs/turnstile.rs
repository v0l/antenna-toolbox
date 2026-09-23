use crate::draw::{Anchor, GOLD, GREEN, PALE, SILVER};
use crate::feed_detail::inline;
use crate::sketch::Sketch;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Scene, row, total, wire};
use antenna_solver::C64;
use antenna_solver::geometry::{Geometry, WireGeometry};

pub static TURNSTILE: Design = Design {
    id: "turnstile",
    name: "Turnstile",
    group: Group::Omni,
    build: Build::Wire,
    gain: "2.1 dBi",
    controls: &[ControlId::Reflector],
    polarisation: "**Circular straight up, horizontal at the horizon.** Two crossed dipoles fed \
                   a quarter cycle apart make a rotating field. Overhead that is circular, which \
                   is what satellites send; around the horizon it is horizontal and nearly \
                   omnidirectional.",
    compute,
};

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let refl = c.ctl(ControlId::Reflector) == 1.0;
    let len = c.P("dipole", 0.47 * lam);
    let below = c.P("reflector spacing", 0.3 * lam);
    let rlen = c.P("reflector", 0.52 * lam);
    let (h, g) = (len / 2.0, 0.004 * lam);
    let mut lines = vec![vec![[-h, 0.0, 0.0], [h, 0.0, 0.0]], vec![[0.0, -h, g], [0.0, h, g]]];
    let mut wires = vec![
        wire(vec![[-h, 0.0, 0.0], [-0.02 * len, 0.0, 0.0]], GOLD, 3.5),
        wire(vec![[0.02 * len, 0.0, 0.0], [h, 0.0, 0.0]], GOLD, 3.5),
        wire(vec![[0.0, -h, g], [0.0, -0.02 * len, g]], GOLD, 3.5),
        wire(vec![[0.0, 0.02 * len, g], [0.0, h, g]], GOLD, 3.5),
    ];
    if refl {
        let r = rlen / 2.0;
        lines.push(vec![[-r, 0.0, -below], [r, 0.0, -below]]);
        lines.push(vec![[0.0, -r, -below + g], [0.0, r, -below + g]]);
        wires.push(wire(vec![[-r, 0.0, -below], [r, 0.0, -below]], SILVER, 3.0));
        wires.push(wire(vec![[0.0, -r, -below + g], [0.0, r, -below + g]], SILVER, 3.0));
    }
    let mut geo = WireGeometry::new(lines, [0.0; 3]);
    geo.sources.push(([0.0, 0.0, g], C64::new(0.0, -1.0)));
    let mut rows = vec![
        row("**each dipole**, tip to tip", c.fmt(len)),
        row("second dipole, fed", "90° behind the first"),
        row("phasing line, 75 Ω, a quarter wave electrical", c.fmt(0.25 * lam)),
    ];
    if refl {
        rows.push(row("**reflector dipoles**, crossed, each", c.fmt(rlen)));
        rows.push(row("reflector below the dipoles", c.fmt(below)));
    }
    rows.push(total("total wire", c.fmt(2.0 * len + if refl { 2.0 * rlen } else { 0.0 })));
    let mut sketch = Sketch::new()
        .wire(&[(-h, 0.0), (h, 0.0)], GOLD, 3.0)
        .dot((0.0, 0.0), GOLD)
        .note((h, 0.02 * lam), "crossed pair, seen from the side", GOLD, Anchor::End)
        .caption(
            if refl {
                "circular, a broad lobe overhead"
            } else {
                "circular overhead, horizontal all round"
            },
            GREEN,
        );
    if refl {
        sketch = sketch.wire(&[(-rlen / 2.0, -below), (rlen / 2.0, -below)], PALE, 3.0).dim(
            (h * 1.2, 0.0),
            (h * 1.2, -below),
            "spacing",
            (8.0, 4.0),
            Anchor::Start,
        );
    }
    Output {
        spec: if refl {
            "~8 dBi overhead · circular · for weather satellites".into()
        } else {
            "~2 dBi overhead · circular up, horizontal omni".into()
        },
        rows,
        scene: Scene {
            wires,
            feed: Some([0.0; 3]),
            omni: !refl,
            omni_y: Some(0.0),
            beam_vec: refl.then_some([0.0, 0.0, 1.0]),
            pol: "polarisation: circular overhead".into(),
            up: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        },
        solve: Geometry::Wire(geo),
        diagram: sketch.render(),
        feed: inline("first dipole", "second dipole, through the phasing line", None),
        notes: "**Two dipoles at right angles, one fed a quarter cycle late.** Seen from \
                above the field rotates, so the antenna hears circular polarisation straight \
                up. Build it by feeding one dipole directly and the other through a quarter \
                wave of coax, both in parallel at the feed; the model drives the second dipole \
                with an ideal 90° source instead.\n\n**Add a reflector for satellites.** On its \
                own a turnstile radiates as much down as up. A second crossed pair, slightly \
                longer, a third of a wave below turns that into a broad lobe overhead, the \
                classic 137 MHz weather satellite antenna. Swap which dipole gets the delay to \
                change the hand of the circular polarisation."
            .into(),
        cut: None,
    }
}
