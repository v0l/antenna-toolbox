pub type Rgba = [u8; 4];
pub type P2 = [f32; 2];

pub const fn hex(v: u32) -> Rgba {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
}

pub const fn hexa(v: u32, alpha: f32) -> Rgba {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, (alpha * 255.0) as u8]
}

pub const GOLD: Rgba = hex(0xe8b23a);
pub const SILVER: Rgba = hex(0xc9d1d6);
pub const GREY: Rgba = hex(0x8a949a);
pub const PALE: Rgba = hex(0xd8dee2);
pub const DIM: Rgba = hex(0x4fa3c7);
pub const GREEN: Rgba = hex(0x6fbf73);
pub const MUTED: Rgba = hex(0x7c8a92);
pub const METAL: Rgba = hex(0x59656d);
pub const BOOM: Rgba = hex(0x2a3136);
pub const INK: Rgba = hex(0x0e1113);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub colour: Rgba,
    pub width: f32,
    pub dash: Option<(f32, f32)>,
}

impl Stroke {
    pub fn new(colour: Rgba, width: f32) -> Self {
        Self { colour, width, dash: None }
    }

    pub fn dashed(colour: Rgba, width: f32, on: f32, off: f32) -> Self {
        Self { colour, width, dash: Some((on, off)) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Sans,
    Mono,
}

#[derive(Clone, Debug)]
pub enum Item {
    Path { pts: Vec<P2>, closed: bool, stroke: Option<Stroke>, fill: Option<Rgba> },
    Circle { c: P2, r: f32, stroke: Option<Stroke>, fill: Option<Rgba> },
    Text { at: P2, text: String, colour: Rgba, size: f32, anchor: Anchor, face: Face },
}

#[derive(Clone, Debug, Default)]
pub struct Drawing {
    pub w: f32,
    pub h: f32,
    pub items: Vec<Item>,
}

fn p(x: f64, y: f64) -> P2 {
    [x as f32, y as f32]
}

impl Drawing {
    pub fn new(w: f64, h: f64) -> Self {
        Self { w: w as f32, h: h as f32, items: Vec::new() }
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, colour: Rgba, width: f64) {
        self.stroke_path(vec![p(x1, y1), p(x2, y2)], false, Stroke::new(colour, width as f32));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dashed(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, colour: Rgba, on: f64, off: f64) {
        self.stroke_path(
            vec![p(x1, y1), p(x2, y2)],
            false,
            Stroke::dashed(colour, 1.0, on as f32, off as f32),
        );
    }

    pub fn polyline(&mut self, pts: &[(f64, f64)], colour: Rgba, width: f64) {
        self.stroke_path(
            pts.iter().map(|&(x, y)| p(x, y)).collect(),
            false,
            Stroke::new(colour, width as f32),
        );
    }

    pub fn polygon(&mut self, pts: &[(f64, f64)], fill: Option<Rgba>, stroke: Option<Stroke>) {
        self.items.push(Item::Path {
            pts: pts.iter().map(|&(x, y)| p(x, y)).collect(),
            closed: true,
            stroke,
            fill,
        });
    }

    pub fn stroke_path(&mut self, pts: Vec<P2>, closed: bool, stroke: Stroke) {
        self.items.push(Item::Path { pts, closed, stroke: Some(stroke), fill: None });
    }

    pub fn rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: Option<Rgba>,
        stroke: Option<Stroke>,
    ) {
        self.polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], fill, stroke);
    }

    pub fn dot(&mut self, cx: f64, cy: f64, r: f64, fill: Rgba) {
        self.items.push(Item::Circle { c: p(cx, cy), r: r as f32, stroke: None, fill: Some(fill) });
    }

    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: Option<Rgba>, stroke: Option<Stroke>) {
        self.items.push(Item::Circle { c: p(cx, cy), r: r as f32, stroke, fill });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn arc(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, a0: f64, a1: f64, stroke: Stroke) {
        let n = ((a1 - a0).abs() * 24.0).ceil().max(8.0) as usize;
        let pts = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f64 / n as f64;
                p(cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect();
        self.stroke_path(pts, false, stroke);
    }

    pub fn text(&mut self, x: f64, y: f64, text: impl Into<String>, colour: Rgba, size: f64) {
        self.text_at(x, y, text, colour, size, Anchor::Start, Face::Sans);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text_at(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        colour: Rgba,
        size: f64,
        anchor: Anchor,
        face: Face,
    ) {
        self.items.push(Item::Text {
            at: p(x, y),
            text: text.into(),
            colour,
            size: size as f32,
            anchor,
            face,
        });
    }

    pub fn label(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        colour: Rgba,
        size: f64,
        anchor: Anchor,
    ) {
        self.text_at(x, y, text, colour, size, anchor, Face::Sans);
    }

    pub fn mono(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        colour: Rgba,
        size: f64,
        anchor: Anchor,
    ) {
        self.text_at(x, y, text, colour, size, anchor, Face::Mono);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dim(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        label: &str,
        dx: f64,
        dy: f64,
        anchor: Anchor,
    ) {
        self.dashed(x1, y1, x2, y2, DIM, 3.0, 3.0);
        self.text_at(
            (x1 + x2) / 2.0 + dx,
            (y1 + y2) / 2.0 + dy,
            label,
            DIM,
            13.0,
            anchor,
            Face::Mono,
        );
    }

    pub fn feed_flag(&mut self, x: f64, y: f64, below: bool) {
        let (dy, ty) = if below { (30.0, 22.0) } else { (-32.0, -34.0) };
        self.circle(x, y, 20.0, None, Some(Stroke::dashed(GREEN, 1.0, 4.0, 4.0)));
        self.line(x + 14.0, y + if below { 14.0 } else { -14.0 }, x + 46.0, y + dy, GREEN, 1.0);
        self.text(x + 50.0, y + ty, "SMA feed, see detail below", GREEN, 12.0);
    }

    pub fn beam_label(&mut self, x: f64, y: f64, text: Option<&str>) {
        self.text_at(
            x,
            y,
            text.unwrap_or("↑ beam direction ↑"),
            GREEN,
            12.0,
            Anchor::Middle,
            Face::Sans,
        );
    }

    pub fn bezier(&mut self, from: (f64, f64), ctrl: (f64, f64), to: (f64, f64), stroke: Stroke) {
        let pts = (0..=32)
            .map(|i| {
                let t = i as f64 / 32.0;
                let u = 1.0 - t;
                p(
                    u * u * from.0 + 2.0 * u * t * ctrl.0 + t * t * to.0,
                    u * u * from.1 + 2.0 * u * t * ctrl.1 + t * t * to.1,
                )
            })
            .collect();
        self.stroke_path(pts, false, stroke);
    }
}
