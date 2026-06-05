//! The picker's visual layer (spec 004) — a Raycast / PowerToys launcher look:
//! an opaque dark panel with a proportional font, an airy layout, a subtle
//! selection, and chip-style footer actions. The window's corners are rounded by
//! an OS region clip (`round_window` in `main.rs`). Rendered by a single
//! `render()` over the egui `Context` so the look stays decoupled +
//! snapshot-tested via egui_kittest.

use eframe::egui;

/// Brand teal (the tray-icon color) — used only as a small title accent.
const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x16, 0xA3, 0x8A);
/// The launcher panel background.
const PANEL: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x1E, 0x20);
/// Subtle neutral highlight behind the selected row.
const SELECTED: egui::Color32 = egui::Color32::from_gray(0x38);
/// Calmer highlight on hover.
const HOVER: egui::Color32 = egui::Color32::from_gray(0x2A);

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

/// Install the launcher theme once: dark, proportional, airy, subtle selection.
pub fn install_theme(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};

    // Append egui's monospace face to the proportional fallback so glyphs the
    // sans lacks (↑↓, etc.) still render instead of tofu boxes.
    let mut fonts = egui::FontDefinitions::default();
    let mono = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
        for f in mono {
            if !prop.contains(&f) {
                prop.push(f);
            }
        }
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();

    // Proportional font everywhere (egui's bundled sans), a touch larger for an
    // airy launcher feel.
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(16.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(15.0, FontFamily::Monospace),
        ),
    ]
    .into();

    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;

    // Subtle neutral selection + calm hover, both rounded; no borders.
    v.selection.bg_fill = SELECTED;
    v.selection.stroke = egui::Stroke::NONE;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
    let r = egui::CornerRadius::same(8);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.inactive.corner_radius = r;

    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.animation_time = 0.08;
    // No text selection in the picker — kills the I-beam cursor on labels.
    style.interaction.selectable_labels = false;

    ctx.set_style(style);
}

fn fmt(color: egui::Color32) -> egui::TextFormat {
    egui::TextFormat {
        color,
        ..Default::default()
    }
}

fn row_job(row: &Row, normal: egui::Color32, dim: egui::Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(&row.name, 0.0, fmt(normal));
    if !row.location.is_empty() {
        job.append(&format!("    {}", row.location), 0.0, fmt(dim));
    }
    job
}

/// Render the picker. Returns what the user did; the caller handles typing,
/// navigation, and Enter. `scroll_to_selected` scrolls the selected row into
/// view (set it when the arrows moved the selection).
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

    // The panel fills the whole (opaque) window; the OS region clip rounds the
    // corners (spec 004 — eframe window transparency wasn't compositing).
    let panel = egui::Frame::NONE
        .fill(PANEL)
        .inner_margin(egui::Margin::same(14));

    egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
        // Footer pinned to the bottom of the panel.
        egui::TopBottomPanel::bottom("atref_footer")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                top: 10,
                ..Default::default()
            }))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    // Chip-style action buttons (subtle fill, arrow cursor).
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::from_gray(0x2A);
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::from_gray(0x3A);
                    if ui.button("enter  insert").clicked() && !rows.is_empty() {
                        action = Action::Accept(selected);
                    }
                    if ui.button("esc  close").clicked() {
                        action = Action::Close;
                    }
                    ui.add_enabled(false, egui::Button::new("↑↓  move"));
                });
            });

        // Header pinned to the top: title + counter + close, then the query.
        egui::TopBottomPanel::top("atref_header")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                bottom: 10,
                ..Default::default()
            }))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("atref").color(ACCENT).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(egui::RichText::new("×").size(18.0))
                            .on_hover_text("close (Esc)")
                            .clicked()
                        {
                            action = Action::Close;
                        }
                        ui.add_space(10.0);
                        ui.weak(counter_text(matches, total));
                    });
                });
                ui.add_space(8.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(query)
                        .font(egui::TextStyle::Heading)
                        .hint_text("Search files…")
                        .desired_width(f32::INFINITY)
                        .frame(false),
                );
                resp.request_focus();
            });

        // Results fill the middle, scrollable.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                if total == 0 {
                    ui.weak("no files indexed");
                    return;
                }
                if rows.is_empty() {
                    ui.weak("no matches");
                    return;
                }
                let row_h = ui.text_style_height(&egui::TextStyle::Body) + 10.0;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, row) in rows.iter().enumerate() {
                            let is_selected = i == selected;
                            let job = row_job(row, normal, dim);
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
    });

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_formats_matches_over_total() {
        // AC6: the counter reads `matches / total`.
        assert_eq!(counter_text(8, 1204), "8 / 1204");
        assert_eq!(counter_text(0, 0), "0 / 0");
    }
}
