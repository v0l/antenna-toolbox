use crate::designs::yagi::{Element, yagi_diagram, yagi_scene};
use crate::draw::{GOLD, GREY};
use crate::feed_detail::inline;
use crate::{Build, ControlId, Ctx, Design, Group, Output, Row, row, total};
use antenna_solver::C64;
use antenna_solver::geometry::{Geometry, Network};

pub static LPDA: Design = Design {
    id: "lpda",
    name: "Log-periodic",
    group: Group::Beam,
    build: Build::Wire,
    gain: "8.9 dBi",
    controls: &[ControlId::Tau, ControlId::Span],
    polarisation: "**Linear, along the elements.** A beam that keeps nearly the same gain, \
                   pattern and impedance across the whole span it is designed for, by handing \
                   the work from element to element as the frequency changes.",
    compute,
};

pub struct Carrel {
    pub sigma: f64,
    pub elements: usize,
    pub longest: f64,
    pub feeder: f64,
}

pub fn carrel(tau: f64, span: f64, f0_lam: f64, dia: f64, r0: f64) -> Carrel {
    let sigma = (0.243 * tau - 0.051).max(0.05);
    let cot_a = 4.0 * sigma / (1.0 - tau);
    let bar = 1.1 + 7.7 * (1.0 - tau).powi(2) * cot_a;
    let bs = span * bar;
    let elements = (1.0 + bs.ln() / (1.0 / tau).ln()).ceil() as usize;
    let longest = f0_lam * span.sqrt() / 2.0;
    let mid = longest * tau.powi(elements as i32 / 2);
    let za = 120.0 * ((mid / dia).ln() - 2.25);
    let s_mean = sigma / tau.sqrt();
    let k = r0 * r0 / (4.0 * s_mean * za);
    let feeder = (k + (k * k + 4.0 * r0 * r0).sqrt()) / 2.0;
    Carrel { sigma, elements, longest, feeder }
}

fn compute(c: &Ctx) -> Output {
    let lam = c.lam;
    let tau = c.ctl(ControlId::Tau);
    let span = c.ctl(ControlId::Span);
    let d = carrel(tau, span, lam, c.wire_dia, 50.0);
    let longest = c.P("longest element", d.longest);
    let mut lens = Vec::with_capacity(d.elements);
    let mut ats = Vec::with_capacity(d.elements);
    let (mut l, mut at) = (longest, 0.0);
    for _ in 0..d.elements {
        lens.push(l);
        ats.push(at);
        at += 2.0 * d.sigma * l;
        l *= tau;
    }
    let boom = ats[d.elements - 1];
    let els: Vec<Element> = (0..d.elements)
        .rev()
        .map(|i| Element {
            len: lens[i],
            at: boom - ats[i],
            label: format!("{}", i + 1),
            colour: if i + 1 == d.elements { GOLD } else { GREY },
            colour3d: if i + 1 == d.elements { GOLD } else { GREY },
            feed: i + 1 == d.elements,
            fold: None,
        })
        .collect();
    let (mut scene, mut geo) = yagi_scene(&els);
    scene.pol = "polarisation: linear, along the elements".into();
    for pair in els.windows(2) {
        let (a, b) = ([0.0, 0.0, -pair[0].at], [0.0, 0.0, -pair[1].at]);
        geo.networks.push((
            a,
            b,
            Network::Line {
                z0: d.feeder,
                length: pair[1].at - pair[0].at,
                crossed: true,
                shunt: [C64::new(0.0, 0.0); 2],
            },
        ));
    }
    let f0 = 299_792.458 / lam;
    let mut rows: Vec<Row> = vec![
        row("band", format!("{:.1} to {:.1} MHz", f0 / span.sqrt(), f0 * span.sqrt())),
        row("τ, each element over the one behind it", format!("{tau:.3}")),
        row("σ, spacing over twice the length", format!("{:.3}", d.sigma)),
        row("**feeder**, crossed between every pair of elements", format!("{:.0} Ω", d.feeder)),
    ];
    for i in 0..d.elements {
        rows.push(row(
            format!("**element {}**, {} from the longest", i + 1, c.fmt(ats[i])),
            c.fmt(lens[i]),
        ));
    }
    rows.push(total("boom length", c.fmt(boom)));
    Output {
        spec: format!("~9 dBi · {} elements · 50 Ω over a {span:.1}:1 band", d.elements),
        rows,
        scene,
        solve: Geometry::Wire(geo),
        diagram: yagi_diagram(&els, c.fmt),
        feed: inline(
            "feeder, one boom",
            "feeder, other boom",
            Some("The coax runs inside one boom tube to the front, its braid bonded to that boom."),
        ),
        notes: "**A row of dipoles, each τ times the one behind it, all driven through one \
                feeder that swaps sides between every pair.** At any frequency only the few \
                elements near half a wave long do the work; the rest are too short or too \
                long to matter. Move the frequency and the active region slides along the \
                boom, so gain and impedance stay nearly constant across the band.\n\n\
                **Designed by Carrel's method.** τ and the span are the controls; σ is set to \
                Carrel's optimum for that τ, and the feeder impedance is chosen for 50 Ω at the \
                input, fed at the short end. Higher τ means more elements, a longer boom and \
                more gain. The feeder is modelled as ideal crossed transmission lines, so the \
                figures hold for a real two-boom build where the elements alternate sides.\n\n\
                The SWR plot here spans the design frequency; the whole band is wider, set by \
                the span control."
            .into(),
        cut: None,
    }
}
