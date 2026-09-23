use crate::draw::{Anchor, Drawing, GREEN, MUTED, Rgba, SILVER};

type Pt = (f64, f64);

#[derive(Default)]
pub struct Sketch {
    lines: Vec<(Vec<Pt>, Rgba, f64)>,
    dims: Vec<(Pt, Pt, String, Pt, Anchor)>,
    notes: Vec<(Pt, String, Rgba, Anchor)>,
    dots: Vec<(Pt, Rgba)>,
    ground: Option<f64>,
    caption: Option<(String, Rgba)>,
    feed: Option<(Pt, bool)>,
    max_h: Option<f64>,
}

impl Sketch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tall(mut self, h: f64) -> Self {
        self.max_h = Some(h);
        self
    }

    pub fn wire(mut self, pts: &[Pt], colour: Rgba, width: f64) -> Self {
        self.lines.push((pts.to_vec(), colour, width));
        self
    }

    pub fn dim(mut self, a: Pt, b: Pt, label: impl Into<String>, off: Pt, anchor: Anchor) -> Self {
        self.dims.push((a, b, label.into(), off, anchor));
        self
    }

    pub fn note(mut self, at: Pt, text: impl Into<String>, colour: Rgba, anchor: Anchor) -> Self {
        self.notes.push((at, text.into(), colour, anchor));
        self
    }

    pub fn dot(mut self, at: Pt, colour: Rgba) -> Self {
        self.dots.push((at, colour));
        self
    }

    pub fn ground(mut self, y: f64) -> Self {
        self.ground = Some(y);
        self
    }

    pub fn caption(mut self, text: impl Into<String>, colour: Rgba) -> Self {
        self.caption = Some((text.into(), colour));
        self
    }

    pub fn feed(mut self, at: Pt, below: bool) -> Self {
        self.feed = Some((at, below));
        self
    }

    pub fn render(self) -> Drawing {
        let w = 640.0;
        let (pad_x, top, bottom) = (110.0, 50.0, if self.caption.is_some() { 60.0 } else { 36.0 });
        let pts = self
            .lines
            .iter()
            .flat_map(|l| l.0.iter().copied())
            .chain(self.dims.iter().flat_map(|d| [d.0, d.1]))
            .chain(self.notes.iter().map(|n| n.0))
            .chain(self.ground.map(|g| (f64::NAN, g)));
        let (mut x0, mut x1, mut y0, mut y1) =
            (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for (x, y) in pts {
            if x.is_finite() {
                x0 = x0.min(x);
                x1 = x1.max(x);
            }
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        let (bw, bh) = ((x1 - x0).max(1e-9), (y1 - y0).max(1e-9));
        let sc = ((w - 2.0 * pad_x) / bw).min(self.max_h.unwrap_or(300.0) / bh);
        let h = bh * sc + top + bottom;
        let cx = w / 2.0 - (x0 + x1) / 2.0 * sc;
        let map = |p: Pt| (cx + p.0 * sc, top + (y1 - p.1) * sc);
        let mut g = Drawing::new(w, h);
        if let Some(gy) = self.ground {
            let (_, y) = map((0.0, gy));
            g.line(40.0, y, w - 40.0, y, SILVER, 2.0);
            g.label(w - 40.0, y + 16.0, "ground", MUTED, 11.0, Anchor::End);
        }
        for (pts, colour, width) in &self.lines {
            let mapped: Vec<Pt> = pts.iter().map(|&p| map(p)).collect();
            g.polyline(&mapped, *colour, *width);
        }
        for (a, b, label, off, anchor) in &self.dims {
            let (a, b) = (map(*a), map(*b));
            g.dim(a.0, a.1, b.0, b.1, label, off.0, off.1, *anchor);
        }
        for (at, colour) in &self.dots {
            let p = map(*at);
            g.dot(p.0, p.1, 3.0, *colour);
        }
        for (at, text, colour, anchor) in &self.notes {
            let p = map(*at);
            g.label(p.0, p.1, text, *colour, 12.0, *anchor);
        }
        if let Some((at, below)) = self.feed {
            let p = map(at);
            g.feed_flag(p.0, p.1, below);
        }
        if let Some((text, colour)) = &self.caption {
            g.label(w / 2.0, h - 14.0, text, *colour, 12.0, Anchor::Middle);
        }
        g
    }
}

pub fn caption_omni() -> (&'static str, Rgba) {
    ("equal in all horizontal directions", GREEN)
}
