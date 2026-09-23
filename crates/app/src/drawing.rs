use antenna_designs::draw::{Anchor, Drawing, Face, Item, Rgba};
use antenna_solver::surface::mesh::triangulate;
use egui::{Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Ui, Vec2};
use egui_bench::theme;

pub fn colour(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn filled(pts: &[Pos2], c: Color32) -> Shape {
    let flat: Vec<[f64; 2]> = pts.iter().map(|q| [q.x as f64, q.y as f64]).collect();
    let mut mesh = egui::Mesh::default();
    for q in pts {
        mesh.colored_vertex(*q, c);
    }
    for t in triangulate(&flat) {
        mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
    }
    Shape::mesh(mesh)
}

pub fn show(ui: &mut Ui, d: &Drawing) {
    let w = ui.available_width().min(760.0);
    let s = w / d.w;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, d.h * s), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, theme::WELL);
    let at = |q: [f32; 2]| Pos2::new(rect.left() + q[0] * s, rect.top() + q[1] * s);
    for item in &d.items {
        match item {
            Item::Path { pts, closed, stroke, fill } => {
                let pts: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
                if let Some(f) = fill {
                    p.add(filled(&pts, colour(*f)));
                }
                if let Some(st) = stroke {
                    let stroke = Stroke::new(st.width * s.max(0.6), colour(st.colour));
                    let mut line = pts;
                    if *closed && let Some(&first) = line.first() {
                        line.push(first);
                    }
                    match st.dash {
                        Some((on, off)) => {
                            p.extend(Shape::dashed_line(&line, stroke, on * s, off * s));
                        }
                        None => {
                            p.add(Shape::line(line, stroke));
                        }
                    }
                }
            }
            Item::Circle { c, r, stroke, fill } => {
                let (c, r) = (at(*c), r * s);
                if let Some(f) = fill {
                    p.circle_filled(c, r, colour(*f));
                }
                if let Some(st) = stroke {
                    let stroke = Stroke::new(st.width * s.max(0.6), colour(st.colour));
                    match st.dash {
                        Some((on, off)) => {
                            let ring: Vec<Pos2> = (0..=72)
                                .map(|i| {
                                    let a = i as f32 / 72.0 * std::f32::consts::TAU;
                                    c + Vec2::new(a.cos(), a.sin()) * r
                                })
                                .collect();
                            p.extend(Shape::dashed_line(&ring, stroke, on * s, off * s));
                        }
                        None => {
                            p.circle_stroke(c, r, stroke);
                        }
                    }
                }
            }
            Item::Text { at: q, text, colour: col, size, anchor, face } => {
                let font = match face {
                    Face::Mono => theme::figure(size * s),
                    Face::Sans => FontId::proportional(size * s),
                };
                let align = match anchor {
                    Anchor::Start => Align2::LEFT_BOTTOM,
                    Anchor::Middle => Align2::CENTER_BOTTOM,
                    Anchor::End => Align2::RIGHT_BOTTOM,
                };
                p.text(at(*q) + Vec2::new(0.0, size * s * 0.25), align, text, font, colour(*col));
            }
        }
    }
}
