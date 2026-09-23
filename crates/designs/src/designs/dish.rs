use crate::draw::{Anchor, Drawing, GOLD, GREY, SILVER, Stroke};
use crate::export::gores;
use crate::feed_detail::inline;
use crate::feeds::{FeedFrame, FeedKind, build_feed};
use crate::{Build, ControlId, Ctx, CutFile, DIPOLE_K, Design, Group, Output, Scene, row, total};
use antenna_solver::geometry::{Geometry, Mesh, WireGeometry};
use antenna_solver::surface::mesh::{merge_meshes, mesh_paraboloid, mesh_profile};

pub static DISH: Design = Design {
    id: "dish",
    name: "Parabolic dish",
    group: Group::Beam,
    build: Build::Both,
    gain: "17 dBi",
    controls: &[ControlId::FeedType, ControlId::FOverD, ControlId::Segments],
    polarisation: "**Linear, set by the feed dipole.** A reflector does not change polarisation, \
                   it only collects. Rotate the feed and the whole antenna rotates with it, which \
                   is why real dishes have a feed you can twist in its clamp.",
    compute,
};

fn disc(radius: f64, z: f64, cell: f64) -> Mesh {
    let stations = ((2.0 * radius / cell).round() as usize).max(4);
    mesh_profile(
        -radius,
        radius,
        stations,
        |x| (radius * radius - x * x).max(0.0).sqrt(),
        z,
        ((2.0 * radius / cell).round() as usize).max(2),
    )
}

fn diagram(diameter: f64, focal: f64, depth: f64) -> Drawing {
    let (w, h) = (640.0, 300.0);
    let sc = ((h - 60.0) / diameter).min((w - 220.0) / (focal + depth));
    let cy = h / 2.0;
    let x0 = 90.0;
    let r = diameter / 2.0 * sc;
    let d = depth * sc;
    let fx = x0 + focal * sc;
    let mut g = Drawing::new(w, h);
    g.bezier((x0 + d, cy - r), (x0 - d * 0.9, cy), (x0 + d, cy + r), Stroke::new(SILVER, 4.0));
    g.line(fx, cy - 16.0, fx, cy + 16.0, GOLD, 4.0);
    g.line(fx + 10.0, cy - 24.0, fx + 10.0, cy + 24.0, GREY, 3.0);
    g.label(fx + 18.0, cy + 40.0, "splash plate", GREY, 12.0, Anchor::Start);
    g.feed_flag(fx, cy, false);
    g.dim(x0, cy, fx, cy, "focal length", 0.0, -8.0, Anchor::Middle);
    g.dim(x0 + d + 30.0, cy - r, x0 + d + 30.0, cy + r, "diameter", 40.0, 4.0, Anchor::Middle);
    g.beam_label(w - 90.0, cy, Some("→ beam"));
    g
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let diameter = c.P("diameter", 3.0 * lam);
    let driven = c.P("feed dipole", DIPOLE_K * lam);
    let kind = FeedKind::from_control(c.ctl(ControlId::FeedType));
    let plate = c.P("splash plate", 0.6 * lam);
    let f_over_d = c.ctl(ControlId::FOverD);
    let segments = c.ctl(ControlId::Segments) as usize;
    let focal = f_over_d * diameter;
    let depth = diameter * diameter / (16.0 * focal);
    let cell = lam / 8.0;
    let feed = build_feed(
        kind,
        lam,
        &FeedFrame { origin: [0.0, 0.0, focal], bore: [0.0, 0.0, -1.0], pol: [1.0, 0.0, 0.0] },
        Some(driven),
    );
    let mesh = merge_meshes(
        &[mesh_paraboloid(diameter, focal, cell, 0.0), disc(plate / 2.0, focal + 0.25 * lam, cell)],
        cell / 100.0,
    );
    let edge_angle = 2.0 * (1.0 / (4.0 * f_over_d)).atan().to_degrees();
    let petals = gores(diameter, focal, segments);

    let mut rows = vec![
        row("**diameter** of the dish", c.fmt(diameter)),
        row("**focal** length", c.fmt(focal)),
        row("f/D ratio", format!("{f_over_d:.2}")),
        row("depth at the centre", c.fmt(depth)),
        row("angle the dish subtends at the feed", format!("{edge_angle:.0}°")),
        row("feed", kind.label()),
    ];
    if kind == FeedKind::Dipole {
        rows.push(row("**feed** dipole, tip to tip", c.fmt(driven)));
    }
    rows.extend([
        row("splash plate diameter", c.fmt(plate)),
        row(
            "**gores** to cut, each",
            format!("{} long, {} wide", c.fmt(petals.length), c.fmt(petals.width)),
        ),
        row("number of gores", segments.to_string()),
        total("facets in the PO model", mesh.triangles.len().to_string()),
    ]);
    let mut geo = WireGeometry::new(feed.lines, feed.feed);
    geo.po = Some(mesh.clone());

    Output {
        spec: "physical optics · reflector · linear".into(),
        rows,
        scene: Scene {
            wires: feed.wires,
            mesh: Some(mesh),
            feed: Some(feed.feed),
            beam_vec: Some([0.0, 0.0, 1.0]),
            pol: feed.pol.into(),
            ..Default::default()
        },
        solve: Geometry::Wire(geo),
        diagram: diagram(diameter, focal, depth),
        feed: inline(
            "feed dipole, one half",
            "feed dipole, other half",
            Some(
                "The feed points back into the dish, with the splash plate a quarter wave behind it.",
            ),
        ),
        notes: format!(
            "{}\n\n**The only design here where the reflector is not solved as unknowns.** A dish \
             of a few wavelengths would need tens of thousands of RWG basis functions, so it is \
             solved by physical optics instead: the feed's field is computed at every facet, the \
             surface current is taken as twice the tangential magnetic field, and those currents \
             are summed into the far field. Facets turned away from the feed are shadowed and \
             contribute nothing.\n\n**The coupling is one way.** The dish shapes the pattern but \
             does not push back on the feed, so the impedance shown is the bare dipole's, not the \
             dish's. For a real dish that is a decent approximation once the feed is more than a \
             wavelength from the surface, and a poor one for a shallow dish where the feed sits \
             close to the metal.\n\n**Expect 30 to 40 percent of the aperture limit with this \
             feed**, not the 50 to 70 percent a real dish reaches. A bare dipole plus splash plate \
             is a crude illuminator: it spills past the rim and its pattern does not taper the way \
             a horn does. The splash plate is worth about 2 dB at 4 λ diameter, which you can check \
             by shrinking it to nothing. Lower f/D means a deeper dish that catches more of the \
             feed pattern, so efficiency rises as you wind f/D down, until the feed starts blocking \
             its own aperture.",
            feed.note
        ),
        cut: Some(CutFile {
            loops: petals.layout,
            circles: vec![],
            note: format!(
                "{segments} gores, {} along the meridian by {} at the rim. Roll each to the curve, \
                 then rivet or solder the seams. A paraboloid cannot be flattened, so each gore \
                 keeps true length along its centre line and true width across, and the error goes \
                 into the seams. More gores means less error and more seams.",
                c.fmt(petals.length),
                c.fmt(petals.width)
            ),
        }),
    }
}
