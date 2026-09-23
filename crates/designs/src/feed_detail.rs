use crate::draw::{Anchor, DIM, Drawing, Face, GOLD, GREY, METAL, MUTED, SILVER, Stroke, hex};

#[derive(Clone, Debug)]
pub enum FeedDetail {
    Inline { left: String, right: String, extra: Option<String> },
    Vertical { top: String, bottom: String, extra: Option<String> },
    Plate,
    GroundPlane,
    Transformer,
}

pub fn inline(left: &str, right: &str, extra: Option<&str>) -> FeedDetail {
    FeedDetail::Inline { left: left.into(), right: right.into(), extra: extra.map(Into::into) }
}

pub fn vertical(top: &str, bottom: &str, extra: Option<&str>) -> FeedDetail {
    FeedDetail::Vertical { top: top.into(), bottom: bottom.into(), extra: extra.map(Into::into) }
}

pub struct Block {
    pub title: &'static str,
    pub drawing: Option<Drawing>,
    pub note: String,
}

const BODY: [u8; 4] = hex(0x3a4349);
const SLEEVE: [u8; 4] = hex(0x2e363b);
const HOLE: [u8; 4] = hex(0x1d2226);

fn with_extra(base: &str, extra: &Option<String>) -> String {
    match extra {
        Some(e) => format!("{base} ^^{e}^^"),
        None => base.to_string(),
    }
}

impl FeedDetail {
    pub fn block(&self) -> Block {
        match self {
            FeedDetail::Inline { left, right, extra } => Block {
                title: "Feed detail: edge-mount or bulkhead SMA, viewed from the side",
                drawing: Some(inline_drawing(left, right)),
                note: with_extra(
                    "It makes no difference which side gets the pin. Keep both leads as short as \
                     the connector allows, since every millimetre of stray lead is length the \
                     antenna did not ask for.",
                    extra,
                ),
            },
            FeedDetail::Vertical { top, bottom, extra } => Block {
                title: "Feed detail: bulkhead SMA, antenna vertical, viewed from the side",
                drawing: Some(vertical_drawing(top, bottom)),
                note: with_extra(
                    "Keep both leads short; stray lead is length the antenna did not ask for.",
                    extra,
                ),
            },
            FeedDetail::Plate => Block {
                title: "Feed detail: bulkhead SMA through the reflector plate, viewed from the side",
                drawing: Some(plate_drawing()),
                note: "The plate **is** the shield. A bulkhead SMA goes through a hole in it and \
                       the nut bonds the body to the metal, so the coax braid, the connector body \
                       and the plate are all one conductor. The centre pin reaches forward across \
                       the gap to one diamond centre. The other diamond centre returns to the \
                       plate by a short strut, which is also what physically holds the element at \
                       the right spacing.\n\n**Yes, the lower diamond is DC-shorted to the plate, \
                       and that is correct.** A biquad is fed unbalanced, exactly like a dipole \
                       with one half bolted to a large piece of metal. Put an ohmmeter across the \
                       SMA and you will read a short; that is the expected result, not a fault. \
                       Both diamonds still radiate."
                    .into(),
            },
            FeedDetail::Transformer => Block {
                title: "Feed detail: step-up transformer in a small box, viewed from the front",
                drawing: Some(transformer_drawing()),
                note: "Wind it as an autotransformer on a type 43 toroid: one continuous winding \
                       of 14 turns, tapped 2 turns up from the cold end. The coax centre pin goes \
                       to the tap, the braid to the cold end, and the radiator to the top of the \
                       winding. The counterpoise joins the braid at the cold end; if you use the \
                       coax braid as the counterpoise instead, fit a choke a short way down the \
                       cable so its length is defined. Many builds add 100 to 150 pF across the \
                       2 turn primary to flatten the SWR on the higher bands."
                    .into(),
            },
            FeedDetail::GroundPlane => Block {
                title: "Feed detail: SMA through the ground plane",
                drawing: None,
                note: "Bulkhead SMA mounted in a hole at the edge of the disc, shield bonded to \
                       the plate by the nut. Centre pin solders to the start of the first turn. \
                       That first quarter turn is the matching section: keep it flat and close to \
                       the plate, then let the wire rise into the helix proper."
                    .into(),
            },
        }
    }
}

fn lead(d: &mut Drawing, pts: &[(f64, f64)], colour: [u8; 4]) {
    d.polyline(pts, colour, 3.0);
}

fn inline_drawing(left: &str, right: &str) -> Drawing {
    let mut d = Drawing::new(640.0, 256.0);
    d.line(40.0, 60.0, 300.0, 60.0, GOLD, 5.0);
    d.line(340.0, 60.0, 600.0, 60.0, GOLD, 5.0);
    d.text(40.0, 44.0, left, GOLD, 12.0);
    d.text_at(600.0, 44.0, right, GOLD, 12.0, Anchor::End, Face::Sans);
    lead(&mut d, &[(320.0, 116.0), (320.0, 88.0), (300.0, 88.0), (300.0, 62.0)], GOLD);
    d.dot(300.0, 60.0, 6.0, [0xe8, 0xb2, 0x3a, 90]);
    d.dot(300.0, 60.0, 3.0, GOLD);
    lead(&mut d, &[(350.0, 116.0), (350.0, 88.0), (340.0, 88.0), (340.0, 62.0)], GREY);
    d.dot(340.0, 60.0, 6.0, [0x8a, 0x94, 0x9a, 90]);
    d.dot(340.0, 60.0, 3.0, GREY);
    d.rect(288.0, 116.0, 64.0, 26.0, Some(BODY), Some(Stroke::new(METAL, 1.5)));
    d.circle(320.0, 129.0, 5.0, Some(HOLE), Some(Stroke::new(METAL, 1.0)));
    d.dot(320.0, 129.0, 2.4, GOLD);
    d.rect(300.0, 142.0, 40.0, 30.0, Some(SLEEVE), Some(Stroke::new(METAL, 1.5)));
    for y in [150.0, 158.0, 166.0] {
        d.line(300.0, y, 340.0, y, METAL, 1.0);
    }
    d.rect(310.0, 172.0, 20.0, 40.0, Some(HOLE), Some(Stroke::new(METAL, 1.5)));
    d.text_at(320.0, 234.0, "coax to radio", MUTED, 12.0, Anchor::Middle, Face::Sans);
    d.line(296.0, 100.0, 210.0, 100.0, GOLD, 1.0);
    d.text_at(206.0, 97.0, "centre pin, hot", GOLD, 12.0, Anchor::End, Face::Sans);
    d.line(354.0, 100.0, 440.0, 100.0, GREY, 1.0);
    d.text(444.0, 97.0, "body / flange, shield", GREY, 12.0);
    d.dashed(300.0, 28.0, 340.0, 28.0, DIM, 3.0, 3.0);
    d.dashed(300.0, 24.0, 300.0, 54.0, DIM, 2.0, 3.0);
    d.dashed(340.0, 24.0, 340.0, 54.0, DIM, 2.0, 3.0);
    d.text_at(320.0, 20.0, "feed gap: keep it small", DIM, 12.0, Anchor::Middle, Face::Mono);
    d.text_at(
        320.0,
        250.0,
        "both halves are driven and identical; only the leads differ",
        MUTED,
        12.0,
        Anchor::Middle,
        Face::Sans,
    );
    d
}

fn vertical_drawing(top: &str, bottom: &str) -> Drawing {
    let mut d = Drawing::new(640.0, 250.0);
    d.line(320.0, 96.0, 320.0, 20.0, GOLD, 5.0);
    d.line(320.0, 154.0, 320.0, 228.0, SILVER, 5.0);
    d.text(336.0, 36.0, top, GOLD, 12.0);
    d.text(336.0, 216.0, bottom, SILVER, 12.0);
    lead(&mut d, &[(250.0, 125.0), (296.0, 125.0), (296.0, 98.0), (320.0, 98.0)], GOLD);
    d.dot(320.0, 98.0, 3.4, GOLD);
    lead(&mut d, &[(250.0, 140.0), (296.0, 140.0), (296.0, 152.0), (320.0, 152.0)], GREY);
    d.dot(320.0, 152.0, 3.4, GREY);
    d.rect(178.0, 118.0, 72.0, 28.0, Some(BODY), Some(Stroke::new(METAL, 1.5)));
    d.circle(214.0, 132.0, 5.0, Some(HOLE), Some(Stroke::new(METAL, 1.0)));
    d.dot(214.0, 132.0, 2.4, GOLD);
    d.rect(140.0, 124.0, 38.0, 16.0, Some(SLEEVE), Some(Stroke::new(METAL, 1.5)));
    d.rect(104.0, 127.0, 36.0, 10.0, Some(HOLE), Some(Stroke::new(METAL, 1.5)));
    d.text_at(98.0, 136.0, "coax to radio", MUTED, 12.0, Anchor::End, Face::Sans);
    d.line(260.0, 110.0, 340.0, 70.0, GOLD, 1.0);
    d.text(344.0, 68.0, "centre pin, hot", GOLD, 11.0);
    d.line(236.0, 148.0, 340.0, 184.0, GREY, 1.0);
    d.text(344.0, 186.0, "body / flange, shield", GREY, 11.0);
    d
}

fn transformer_drawing() -> Drawing {
    let mut d = Drawing::new(640.0, 280.0);
    let (x, top, bottom) = (330.0, 60.0, 200.0);
    d.rect(250.0, 40.0, 160.0, 180.0, Some(SLEEVE), Some(Stroke::new(METAL, 1.5)));
    let turns = 14;
    let pitch = (bottom - top) / turns as f64;
    for i in 0..turns {
        let y = bottom - (i as f64 + 0.5) * pitch;
        d.arc(
            x,
            y,
            12.0,
            pitch / 2.0,
            -std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_2,
            Stroke::new(GOLD, 2.0),
        );
    }
    d.line(x, top - pitch / 2.0, x, top, GOLD, 2.0);
    d.line(x, bottom, x, bottom + 4.0, GOLD, 2.0);
    let tap = bottom - 2.0 * pitch;
    d.dot(x, tap, 3.4, GOLD);
    d.dot(x, bottom + 4.0, 3.4, GREY);
    lead(&mut d, &[(x, top - pitch / 2.0), (x, 50.0), (600.0, 50.0)], GOLD);
    d.text_at(
        600.0,
        36.0,
        "radiator, from the top of the winding",
        GOLD,
        12.0,
        Anchor::End,
        Face::Sans,
    );
    lead(&mut d, &[(x, bottom + 4.0), (40.0, bottom + 4.0)], GREY);
    d.text(40.0, bottom - 8.0, "counterpoise, from the cold end", GREY, 12.0);
    d.rect(262.0, 226.0, 40.0, 22.0, Some(BODY), Some(Stroke::new(METAL, 1.5)));
    d.rect(272.0, 248.0, 20.0, 26.0, Some(HOLE), Some(Stroke::new(METAL, 1.5)));
    lead(&mut d, &[(282.0, 226.0), (282.0, tap), (x, tap)], GOLD);
    lead(&mut d, &[(296.0, 226.0), (296.0, bottom + 4.0)], GREY);
    d.text_at(262.0, 262.0, "coax to radio", MUTED, 12.0, Anchor::End, Face::Sans);
    d.line(x + 16.0, tap, 440.0, tap, GOLD, 1.0);
    d.text(444.0, tap + 4.0, "tap, 2 turns up: centre pin", GOLD, 12.0);
    d.line(x + 16.0, (top + tap) / 2.0, 440.0, (top + tap) / 2.0, MUTED, 1.0);
    d.text(444.0, (top + tap) / 2.0 + 4.0, "14 turns in all, on a type 43 toroid", MUTED, 12.0);
    d.line(300.0, bottom + 4.0, 440.0, bottom + 24.0, GREY, 1.0);
    d.text(444.0, bottom + 28.0, "cold end: braid and counterpoise", GREY, 12.0);
    d
}

fn plate_drawing() -> Drawing {
    let mut d = Drawing::new(640.0, 250.0);
    d.line(180.0, 26.0, 180.0, 224.0, METAL, 6.0);
    d.text_at(172.0, 20.0, "reflector plate", GREY, 12.0, Anchor::Middle, Face::Sans);
    d.rect(96.0, 112.0, 84.0, 26.0, Some(SLEEVE), Some(Stroke::new(METAL, 1.5)));
    d.rect(168.0, 104.0, 16.0, 42.0, Some(BODY), Some(Stroke::new(METAL, 1.5)));
    d.text_at(90.0, 129.0, "coax", MUTED, 12.0, Anchor::End, Face::Sans);
    d.text_at(
        176.0,
        166.0,
        "nut bonds the shield to the plate",
        GREY,
        11.0,
        Anchor::Middle,
        Face::Sans,
    );
    lead(&mut d, &[(184.0, 112.0), (330.0, 112.0), (330.0, 44.0)], GOLD);
    d.dot(330.0, 112.0, 3.2, GOLD);
    lead(&mut d, &[(180.0, 168.0), (330.0, 168.0), (330.0, 206.0)], GREY);
    d.dot(330.0, 168.0, 3.2, GREY);
    d.dot(180.0, 168.0, 4.0, GREY);
    d.line(330.0, 44.0, 330.0, 26.0, GOLD, 3.0);
    d.line(330.0, 206.0, 330.0, 224.0, GOLD, 3.0);
    d.text(346.0, 40.0, "upper diamond, on the centre pin", GOLD, 12.0);
    d.text(346.0, 212.0, "lower diamond, on the shield", GOLD, 12.0);
    d.text(346.0, 116.0, "centre pin, hot", GOLD, 11.0);
    d.text(346.0, 172.0, "strut to the plate: shield, and a DC short", GREY, 11.0);
    d.dashed(186.0, 84.0, 330.0, 84.0, DIM, 3.0, 3.0);
    d.dashed(186.0, 78.0, 186.0, 112.0, DIM, 2.0, 3.0);
    d.dashed(330.0, 78.0, 330.0, 112.0, DIM, 2.0, 3.0);
    d.text_at(258.0, 76.0, "gap", DIM, 12.0, Anchor::Middle, Face::Mono);
    d
}
