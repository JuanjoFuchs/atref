//! The picker's visual layer (spec 003) — a console / accent style. Rendered by
//! a single `render()` (over the egui `Context`, using pinned top/bottom panels)
//! so the look is decoupled from the App's state/OS wiring and snapshot-tested
//! headlessly via `egui_kittest`.

use eframe::egui;

/// Teal accent (the tray-icon color): the title + the selected-row fill.
const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x16, 0xA3, 0x8A);
/// Dark console background.
const PANEL: egui::Color32 = egui::Color32::from_rgb(0x12, 0x12, 0x12);
/// High-contrast text drawn on the teal selected row.
const ON_ACCENT: egui::Color32 = egui::Color32::from_rgb(0x05, 0x14, 0x11);

/// One result row's display strings.
pub struct Row {
    pub name: String,
    pub location: String,
}

/// What the user did this frame in the picker.
pub enum Action {
    None,
    Accept(usize),
    Close,
}

/// The `matches / total` counter text.
pub fn counter_text(matches: usize, total: usize) -> String {
    format!("{matches} / {total}")
}

/// Install the console theme once: dark background, monospace everywhere, teal
/// selection, a calm (un-animated, subtle) hover. Call from `App::new`.
pub fn install_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Monospace everywhere — remap every text style to the monospace family.
    for (_, font_id) in style.text_styles.iter_mut() {
        font_id.family = egui::FontFamily::Monospace;
    }

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.selection.bg_fill = ACCENT;
    // Calm hover: a subtle dark wash, no border, no fade animation.
    let hover = egui::Color32::from_rgb(0x20, 0x26, 0x28);
    visuals.widgets.hovered.weak_bg_fill = hover;
    visuals.widgets.hovered.bg_fill = hover;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.active.bg_stroke = egui::Stroke::NONE;

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.animation_time = 0.0;

    ctx.set_style(style);
}

fn fmt(color: egui::Color32) -> egui::TextFormat {
    egui::TextFormat {
        color,
        ..Default::default()
    }
}

fn row_job(
    row: &Row,
    selected: bool,
    normal: egui::Color32,
    dim: egui::Color32,
) -> egui::text::LayoutJob {
    let (name_c, loc_c) = if selected {
        (ON_ACCENT, ON_ACCENT)
    } else {
        (normal, dim)
    };
    let mut job = egui::text::LayoutJob::default();
    job.append(&row.name, 0.0, fmt(name_c));
    if !row.location.is_empty() {
        job.append(&format!("    {}", row.location), 0.0, fmt(loc_c));
    }
    job
}

/// Render the picker over the whole window. Returns what the user did. Typing,
/// navigation, and Enter are handled by the caller; this draws + reports a row
/// click or a close-button click. `scroll_to_selected` scrolls the selected row
/// into view (set it when the arrows moved the selection).
pub fn render(
    ctx: &egui::Context,
    query: &mut String,
    rows: &[Row],
    selected: usize,
    scroll_to_selected: bool,
    matches: usize,
    total: usize,
) -> Action {
    let mut action = Action::None;
    let normal = ctx.style().visuals.text_color();
    let dim = ctx.style().visuals.weak_text_color();

    // Footer pinned to the bottom.
    egui::TopBottomPanel::bottom("atref_footer").show(ctx, |ui| {
        ui.add_space(2.0);
        ui.weak("enter insert · esc cancel · ↑↓ move · click outside to close");
    });

    // Header pinned to the top: title + counter + close, then the query line.
    egui::TopBottomPanel::top("atref_header").show(ctx, |ui| {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("atref").color(ACCENT).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Label::new("✕").sense(egui::Sense::click()))
                    .on_hover_text("close (Esc)")
                    .clicked()
                {
                    action = Action::Close;
                }
                ui.add_space(8.0);
                ui.weak(counter_text(matches, total));
            });
        });
        ui.horizontal(|ui| {
            ui.label(">");
            let resp = ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text("type to filter…")
                    .desired_width(f32::INFINITY)
                    .frame(false),
            );
            resp.request_focus();
        });
        ui.add_space(2.0);
    });

    // Rows fill the rest, scrollable.
    egui::CentralPanel::default().show(ctx, |ui| {
        if total == 0 {
            ui.weak("no files indexed");
            return;
        }
        if rows.is_empty() {
            ui.weak("no matches");
            return;
        }
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, row) in rows.iter().enumerate() {
                    let is_selected = i == selected;
                    let job = row_job(row, is_selected, normal, dim);
                    let resp = ui.add_sized(
                        [ui.available_width(), row_h],
                        egui::Button::selectable(is_selected, job),
                    );
                    if resp.clicked() {
                        action = Action::Accept(i);
                    }
                    if is_selected && scroll_to_selected {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
    });

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_formats_matches_over_total() {
        // AC4: the counter reads `matches / total`.
        assert_eq!(counter_text(8, 1204), "8 / 1204");
        assert_eq!(counter_text(0, 0), "0 / 0");
    }
}
