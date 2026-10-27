//! The Re programs' shared look (ReEtched's dark, compact theme), plus the
//! launcher's cards and buttons.

use egui::{Color32, CornerRadius, FontFamily, FontId, RichText, Stroke, TextStyle, vec2};
use egui_material_icons::MaterialIcon;

pub const ACCENT: Color32 = Color32::from_rgb(0x4a, 0x90, 0xd9);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x5c, 0x9e, 0xe2);
pub const PANEL: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2c);
pub const HEADER: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);
pub const STRIP: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
pub const CARD: Color32 = Color32::from_rgb(0x27, 0x27, 0x27);
pub const GAP: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
pub const ROW_HOVER: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
pub const ROW_SELECTED: Color32 = Color32::from_rgb(0x34, 0x3a, 0x42);
pub const TEXT: Color32 = Color32::from_rgb(0xd4, 0xd4, 0xd4);
pub const STRONG: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf0);
pub const WEAK: Color32 = Color32::from_rgb(0x8c, 0x8c, 0x8c);
pub const OK: Color32 = Color32::from_rgb(0x6a, 0xbf, 0x69);
pub const WARN: Color32 = Color32::from_rgb(0xd9, 0xa6, 0x4e);
pub const ERROR: Color32 = Color32::from_rgb(0xe0, 0x6c, 0x5c);

pub fn apply(ctx: &egui::Context) {
    egui_material_icons::initialize(ctx);
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        let radius = CornerRadius::same(3);
        let v = &mut s.visuals;
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.extreme_bg_color = Color32::from_rgb(0x1e, 0x1e, 0x1e);
        v.faint_bg_color = Color32::from_rgb(0x30, 0x30, 0x30);
        v.window_stroke = Stroke::new(1.0, GAP);
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(3);
        v.selection.bg_fill = Color32::from_rgb(0x2b, 0x52, 0x7c);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;

        let w = &mut v.widgets;
        let shade =
            |fill: Color32, stroke: Color32, text: Color32, wv: &mut egui::style::WidgetVisuals| {
                wv.bg_fill = fill;
                wv.weak_bg_fill = fill;
                wv.bg_stroke = Stroke::new(1.0, stroke);
                wv.fg_stroke = Stroke::new(1.0, text);
                wv.corner_radius = radius;
            };
        shade(PANEL, GAP, TEXT, &mut w.noninteractive);
        shade(
            Color32::from_rgb(0x3a, 0x3a, 0x3a),
            Color32::from_rgb(0x45, 0x45, 0x45),
            TEXT,
            &mut w.inactive,
        );
        shade(
            Color32::from_rgb(0x46, 0x46, 0x46),
            Color32::from_rgb(0x5a, 0x5a, 0x5a),
            Color32::WHITE,
            &mut w.hovered,
        );
        shade(
            Color32::from_rgb(0x52, 0x52, 0x52),
            ACCENT,
            Color32::WHITE,
            &mut w.active,
        );
        shade(
            Color32::from_rgb(0x3e, 0x3e, 0x3e),
            Color32::from_rgb(0x55, 0x55, 0x55),
            TEXT,
            &mut w.open,
        );

        s.spacing.item_spacing = vec2(6.0, 5.0);
        s.spacing.button_padding = vec2(8.0, 3.0);
        s.spacing.interact_size.y = 22.0;
        s.text_styles = [
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(20.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}

/// Small, bold, upper-case section title (ReEtched's panel headers).
pub fn header(text: &str) -> RichText {
    RichText::new(text).size(11.0).strong().color(WEAK)
}

/// A bordered block grouping one section.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, GAP))
        .corner_radius(4)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// "<icon>  text"
pub fn label(icon: MaterialIcon, text: &str) -> String {
    format!("{}  {text}", icon.codepoint)
}

/// The main action (Launch / Install / Update): filled with the accent color.
pub fn primary(ui: &mut egui::Ui, enabled: bool, icon: MaterialIcon, text: &str) -> egui::Response {
    let fill = if enabled {
        ACCENT
    } else {
        Color32::from_rgb(0x3a, 0x3a, 0x3a)
    };
    let color = if enabled { Color32::WHITE } else { WEAK };
    let resp = ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label(icon, text))
                .size(14.0)
                .strong()
                .color(color),
        )
        .fill(fill)
        .stroke(Stroke::NONE)
        .min_size(vec2(150.0, 34.0)),
    );
    if resp.hovered() && enabled {
        ui.painter()
            .rect_filled(resp.rect, 3, ACCENT_HOVER.gamma_multiply(0.25));
    }
    resp
}

/// A secondary action next to the main one.
pub fn secondary(
    ui: &mut egui::Ui,
    enabled: bool,
    icon: MaterialIcon,
    text: &str,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label(icon, text)).size(13.0)).min_size(vec2(0.0, 34.0)),
    )
}

/// A frameless icon button (toolbars).
pub fn icon_button(ui: &mut egui::Ui, icon: MaterialIcon, tip: &str) -> egui::Response {
    ui.add(
        egui::Button::new(icon.rich_text().size(18.0).color(TEXT))
            .frame(false)
            .min_size(vec2(28.0, 28.0)),
    )
    .on_hover_text(tip)
}
