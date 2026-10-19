//! Small adapter for CADCraft's visual language, without loading the editor UI.
//! Palette, typography and spacing follow `crates/ui-egui/src/theme.rs`.
use egui::{Color32, FontId, Stroke};

pub const CHROME: Color32 = Color32::from_rgb(0x3b, 0x44, 0x53);
pub const CHROME_DARK: Color32 = Color32::from_rgb(0x31, 0x38, 0x44);
pub const PANEL: Color32 = Color32::from_rgb(0x3c, 0x44, 0x52);
pub const TAB_ACTIVE: Color32 = Color32::from_rgb(0x4e, 0x5a, 0x6e);
pub const CONTROL: Color32 = Color32::from_rgb(0x50, 0x5a, 0x6d);
pub const CONTROL_HOVER: Color32 = Color32::from_rgb(0x5c, 0x67, 0x7c);
pub const BORDER: Color32 = Color32::from_rgb(0x23, 0x29, 0x30);
pub const TEXT: Color32 = Color32::from_rgb(0xd7, 0xdb, 0xe0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xa3, 0xaa, 0xb4);
pub const ICON: Color32 = Color32::from_rgb(0xcf, 0xd4, 0xdb);
pub const ICON_ACCENT: Color32 = Color32::from_rgb(0x4f, 0xa3, 0xf7);
pub const ACCENT: Color32 = Color32::from_rgb(0x3d, 0x8b, 0xfd);
pub const WARN: Color32 = Color32::from_rgb(0xff, 0xd0, 0x80);
pub const CANVAS: Color32 = Color32::from_rgb(0x22, 0x28, 0x2f);
pub const PAPER: Color32 = Color32::WHITE;

// Original MIT OR Apache-2.0 viewer artwork: engraved peregrine falcon on cyan.
// The editor's icon and ArtCraft marks are separate.
pub fn app_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/viewer-icon/viewer-256.png")).ok()
}

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = CHROME;
    v.extreme_bg_color = CHROME_DARK;
    v.faint_bg_color = CHROME_DARK;
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.fg_stroke.color = TEXT;
    v.widgets.noninteractive.bg_stroke.color = BORDER;
    v.widgets.inactive.bg_fill = CONTROL;
    v.widgets.inactive.weak_bg_fill = CONTROL;
    v.widgets.inactive.fg_stroke.color = TEXT;
    v.widgets.hovered.bg_fill = CONTROL_HOVER;
    v.widgets.hovered.weak_bg_fill = CONTROL_HOVER;
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.selection.bg_fill = ACCENT;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.menu_corner_radius = egui::CornerRadius::same(5);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
        s.spacing.interact_size.y = 24.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(12.5));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(12.5));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(11.5));
    });
}
