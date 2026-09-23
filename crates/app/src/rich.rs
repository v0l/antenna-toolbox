use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Ui};
use egui_bench::theme;

pub fn job(text: &str, size: f32, base: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut bold = false;
    let mut warn = false;
    let mut rest = text;
    while !rest.is_empty() {
        let next = [rest.find("**"), rest.find("^^")].into_iter().flatten().min();
        let (chunk, marker) = match next {
            Some(i) => (&rest[..i], Some(&rest[i..i + 2])),
            None => (rest, None),
        };
        if !chunk.is_empty() {
            let colour = if warn {
                theme::READOUT
            } else if bold {
                theme::VALUE
            } else {
                base
            };
            let font =
                if bold { theme::legend_font(size + 0.5) } else { FontId::proportional(size) };
            job.append(
                chunk,
                0.0,
                TextFormat { font_id: font, color: colour, ..Default::default() },
            );
        }
        match marker {
            Some("**") => bold = !bold,
            Some(_) => warn = !warn,
            None => break,
        }
        rest = &rest[chunk.len() + 2..];
    }
    job
}

pub fn prose(ui: &mut Ui, text: &str) {
    for (i, para) in text.split("\n\n").enumerate() {
        if i > 0 {
            ui.add_space(6.0);
        }
        let j = job(para, 13.0, theme::LEGEND, ui.available_width());
        ui.add(egui::Label::new(j).wrap());
    }
}

pub fn inline(ui: &mut Ui, text: &str, size: f32, base: Color32) -> egui::Response {
    let width = ui.available_width();
    ui.add(egui::Label::new(job(text, size, base, width)).wrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_split_into_sections() {
        let j = job("plain **bold** and ^^warn^^ tail", 13.0, Color32::GRAY, 400.0);
        let texts: Vec<&str> =
            j.sections.iter().map(|s| &j.text[s.byte_range.start.0..s.byte_range.end.0]).collect();
        assert_eq!(texts, ["plain ", "bold", " and ", "warn", " tail"]);
    }
}
