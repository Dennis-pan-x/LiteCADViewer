//! The viewer subset of CADCraft's original 24x24 line icons.
//! Geometry adapted from `crates/ui-egui/src/icons.rs` (MIT OR Apache-2.0).
use crate::theme;
use craft_ui::buttons::{ButtonVisuals, IconButton, IconButtonStyle};
use egui::{Color32, Pos2, Rect, Stroke};

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Fit,
    Layers,
    Measure,
    Export,
    Sun,
}

struct Pen<'a> {
    painter: &'a egui::Painter,
    rect: Rect,
    base: Color32,
    accent: Color32,
}
impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + egui::vec2(x, y) * (self.rect.width() / 24.0)
    }
    fn line(&self, points: &[(f32, f32)], closed: bool, color: Color32) {
        let mut points: Vec<_> = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        if closed && let Some(first) = points.first().copied() {
            points.push(first);
        }
        self.painter.line(points, Stroke::new((self.rect.width() / 16.0).clamp(1.0, 1.6), color));
    }
    fn arrow(&self, x: f32, y: f32, direction: f32) {
        let points = [(x, y), (x - direction * 3.0, y - 1.4), (x - direction * 3.0, y + 1.4)];
        self.painter.add(egui::Shape::convex_polygon(points.iter().map(|(x, y)| self.at(*x, *y)).collect(), self.accent, Stroke::NONE));
    }
    fn paint(&self, icon: Icon) {
        let (b, a) = (self.base, self.accent);
        match icon {
            Icon::Open => {
                self.line(&[(3., 6.), (9., 6.), (11., 8.), (20., 8.), (20., 19.), (3., 19.)], true, b);
                self.line(&[(3., 19.), (6., 11.), (22., 11.), (20., 19.)], false, b);
            }
            Icon::Fit => {
                self.painter.circle_stroke(self.at(10., 10.), self.rect.width() / 4.0, Stroke::new(1.4, b));
                self.line(&[(14.5, 14.5), (20., 20.)], false, b);
                self.line(&[(7., 10.), (13., 10.)], false, a);
                self.line(&[(10., 7.), (10., 13.)], false, a);
            }
            Icon::Layers => {
                for (index, y) in [5., 10., 15.].into_iter().enumerate() {
                    self.line(&[(3., y + 2.), (12., y - 2.), (21., y + 2.), (12., y + 6.)], true, if index == 0 { a } else { b });
                }
            }
            Icon::Measure => {
                self.line(&[(3., 15.), (21., 15.), (21., 20.), (3., 20.)], true, b);
                for x in [6., 9., 12., 15., 18.] {
                    self.line(&[(x, 15.), (x, 17.5)], false, b);
                }
                self.line(&[(3., 8.), (21., 8.)], false, a);
                self.arrow(3., 8., -1.);
                self.arrow(21., 8., 1.);
            }
            Icon::Export => {
                self.line(&[(5., 3.), (14., 3.), (18., 7.), (18., 21.), (5., 21.)], true, b);
                self.line(&[(11., 13.), (21., 13.)], false, a);
                self.arrow(23., 13., 1.);
            }
            Icon::Sun => {
                let c = Color32::from_rgb(0xf5, 0xd0, 0x3b).gamma_multiply(if self.base == theme::ICON { 1.0 } else { 0.4 });
                self.painter.circle_filled(self.at(12., 12.), self.rect.width() / 6.0, c);
                for i in 0..8 {
                    let angle = (i as f32 * 45.0).to_radians();
                    self.line(&[(12. + 6. * angle.cos(), 12. + 6. * angle.sin()), (12. + 9. * angle.cos(), 12. + 9. * angle.sin())], false, c);
                }
            }
        }
    }
}

pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let visuals = |fill| ButtonVisuals { fill, stroke: Stroke::NONE, icon: theme::ICON };
        let style = IconButtonStyle {
            normal: visuals(Color32::TRANSPARENT),
            hovered: visuals(theme::CONTROL_HOVER.gamma_multiply(0.6)),
            pressed: None,
            selected: visuals(theme::TAB_ACTIVE),
            corner_radius: egui::CornerRadius::same(3),
            focus_stroke: Stroke::new(1.0, theme::ACCENT),
        };
        let galley = ui.painter().layout_no_wrap(label.into(), egui::FontId::proportional(12.5), theme::TEXT);
        let size = egui::vec2(34.0 + galley.size().x + 8.0, 30.0);
        IconButton::new(label, size, &style).selected(selected).show(ui, |ui, rect, _| {
            let alpha = |c: Color32| if ui.is_enabled() { c } else { c.gamma_multiply(0.4) };
            let icon_rect = Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), egui::vec2(22.0, 22.0));
            Pen { painter: ui.painter(), rect: icon_rect, base: alpha(theme::ICON), accent: alpha(theme::ICON_ACCENT) }.paint(icon);
            ui.painter().galley(egui::pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0), galley, alpha(theme::TEXT));
        })
    })
    .inner
}
