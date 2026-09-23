use antenna_solver::C64;

const UNBALANCED: [&str; 5] = ["gp", "discone", "discmono", "coil", "helix"];

pub fn is_balanced(id: &str) -> bool {
    !UNBALANCED.contains(&id)
}

pub const REAL_LINES: [(f64, &str); 7] = [
    (25.0, "two 50 Ω in parallel"),
    (37.5, "two 75 Ω in parallel"),
    (50.0, "RG-58, RG-142, RG-316"),
    (75.0, "RG-59, RG-6, satellite coax"),
    (93.0, "RG-62"),
    (100.0, "two 50 Ω in series"),
    (150.0, "two 75 Ω in series"),
];

pub fn nearest_line(z: f64) -> (f64, &'static str) {
    REAL_LINES.iter().copied().min_by(|a, b| (a.0 - z).abs().total_cmp(&(b.0 - z).abs())).unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    Direct,
    Choke,
    Balun4,
    Qwt,
    ChokeQwt,
    Balun4Qwt,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub ideal: f64,
    pub pick: (f64, &'static str),
    pub len_vf66: f64,
    pub len_vf82: f64,
}

#[derive(Clone, Debug)]
pub struct MatchPlan {
    pub kind: MatchKind,
    pub headline: &'static str,
    pub balun: String,
    pub line: Option<Line>,
    pub caveat: Option<&'static str>,
    use4: bool,
}

fn through_line(z: C64, zt: f64, f: f64, f0: f64) -> C64 {
    let t = (std::f64::consts::FRAC_PI_2 * (f / f0)).tan();
    let den = C64::new(zt - z.im * t, z.re * t);
    if !t.is_finite() || den.norm_sqr() < 1e-12 {
        return zt * zt / z;
    }
    zt * (z + C64::new(0.0, zt * t)) / den
}

pub fn swr_of_50(z: C64, z0: f64) -> f64 {
    let g = ((z - z0).norm_sqr() / (z + z0).norm_sqr()).sqrt();
    if g >= 0.9999 { 99.0 } else { (1.0 + g) / (1.0 - g) }
}

fn mm(v: f64) -> String {
    if v < 100.0 { format!("{v:.1} mm") } else { format!("{v:.0} mm") }
}

fn choke_advice(lam_mm: f64) -> String {
    let why = "Balanced antenna, unbalanced cable. Coax carries its signal on the inside of the \
               braid, but a balanced antenna lets some current escape onto the outside, so the \
               feedline joins in and radiates. You get a pattern that is not the one above, and an \
               SWR that changes when you move the cable. A choke blocks that outside current and \
               leaves the signal inside untouched. Fit it at the feed point, not down by the radio. ";
    if lam_mm > 3000.0 {
        format!(
            "{why}Wind 8 to 12 turns of the coax into a coil about {} across, right where the \
             cable meets the antenna, and tape it. A stack of type 31 ferrite cores over the cable \
             does the same job in less space.",
            mm(lam_mm * 0.02)
        )
    } else if lam_mm > 600.0 {
        format!(
            "{why}Wind 5 or 6 turns of the coax into a coil about {} across at the feed point, \
             or clip two or three ferrite cores over the cable there. Either works at this \
             frequency.",
            mm(lam_mm * 0.05)
        )
    } else {
        format!(
            "{why}At this frequency the cable cannot be coiled: a useful coil would be about {} \
             across, tighter than thin coax will bend without damage, and ferrite has mostly \
             given up by here. Build a sleeve balun instead. Slide a copper tube, or a piece of \
             braid pulled off some scrap coax, over the outside of the cable: {} long, open at the \
             antenna end, soldered all round to the braid at the far end. That quarter-wave \
             sleeve is what stops current coming back down the outside.",
            mm(lam_mm * 0.1),
            mm(lam_mm * 0.2375)
        )
    }
}

pub fn plan_match(id: &str, z: C64, lam_mm: f64, z0: f64) -> MatchPlan {
    let balanced = is_balanced(id);
    let reactive = z.im.abs() > 0.3 * z.re.max(10.0);
    let use4 = balanced && (z.re / 4.0 - z0).abs() < (z.re - z0).abs() && z.re > 2.0 * z0;
    let after_balun = if use4 { z.re / 4.0 } else { z.re };
    let needs_line = after_balun < 0.8 * z0 || after_balun > 1.25 * z0;

    let balun = if balanced {
        if use4 {
            format!(
                "The resistance is high enough that a 4:1 balun earns its place, and it fixes the \
                 balance at the same time. Build it from an extra loop of coax joining the two \
                 sides of the feed: {} of solid PE cable, or {} of foam, measured between the \
                 solder joints. The loop quarters the impedance and flips the phase, which is \
                 what makes both halves of the antenna see the same drive.",
                mm(lam_mm / 2.0 * 0.66),
                mm(lam_mm / 2.0 * 0.82)
            )
        } else {
            choke_advice(lam_mm)
        }
    } else {
        "Fed against its own ground plane, so it takes coax directly and needs no balun. Worth \
         adding a ferrite or two anyway if the cable has to run back past the antenna, since a \
         feedline lying in the field will still pick up current."
            .to_string()
    };

    let mut line = needs_line.then(|| {
        let ideal = (after_balun.max(1.0) * z0).sqrt();
        let quarter = lam_mm / 4.0;
        Line { ideal, pick: nearest_line(ideal), len_vf66: quarter * 0.66, len_vf82: quarter * 0.82 }
    });

    let trial = |with_line: bool, line: &Option<Line>| {
        let mut zz = if use4 { z / 4.0 } else { z };
        if let (true, Some(l)) = (with_line, line) {
            zz = through_line(zz, l.pick.0, 1.0, 1.0);
        }
        swr_of_50(zz, z0)
    };
    let helps = line.is_some() && trial(true, &line) < trial(false, &line) * 0.95;
    if !helps {
        line = None;
    }

    let caveat = reactive.then_some(if line.is_some() {
        "There is real reactance left at this size, so trim to resonance with the tune button and \
         the transformer will do better than the figure below."
    } else {
        "This one is off resonance, and reactance is what is spoiling the match rather than \
         resistance. A transformer cannot help with that, and would make it worse here, so none \
         is offered. Tune it to resonance first, then come back."
    });

    let kind = match (balanced, use4, line.is_some()) {
        (false, _, true) => MatchKind::Qwt,
        (false, _, false) => MatchKind::Direct,
        (true, true, true) => MatchKind::Balun4Qwt,
        (true, true, false) => MatchKind::Balun4,
        (true, false, true) => MatchKind::ChokeQwt,
        (true, false, false) => MatchKind::Choke,
    };
    let headline = match kind {
        MatchKind::Direct => "Straight onto 50 Ω coax",
        MatchKind::Choke => "1:1 choke, then straight onto 50 Ω coax",
        MatchKind::Balun4 => "4:1 balun, then straight onto 50 Ω coax",
        MatchKind::Qwt => "Quarter-wave transformer",
        MatchKind::ChokeQwt => "1:1 choke and a quarter-wave transformer",
        MatchKind::Balun4Qwt => "4:1 balun and a quarter-wave transformer",
    };
    MatchPlan { kind, headline, balun, line, caveat, use4 }
}

impl MatchPlan {
    pub fn apply(&self, z: C64, f: f64, f0: f64) -> C64 {
        let zz = if self.use4 { z / 4.0 } else { z };
        match &self.line {
            Some(l) => through_line(zz, l.pick.0, f, f0),
            None => zz,
        }
    }
}
