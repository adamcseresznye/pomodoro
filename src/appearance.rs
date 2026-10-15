use eframe::egui::{self, Color32, Pos2, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Background {
    #[default]
    Solid,
    Gradient,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Texture {
    #[default]
    None,
    Grain,
    Linen,
    Dots,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub dark: bool,
    pub background: Background,
    pub color: [u8; 3],
    pub second: [u8; 3],
    pub texture: Texture,
    /// Progress ring and primary buttons during focus.
    pub clock: [u8; 3],
    /// Progress ring and primary buttons during breaks.
    pub clock_break: [u8; 3],
}

/// Named clock colour pairs: (name, focus, break).
pub const CLOCK_COLORS: [(&str, [u8; 3], [u8; 3]); 6] = [
    ("Ocean", [62, 114, 158], [60, 117, 90]),
    ("Ember", [190, 92, 48], [52, 124, 120]),
    ("Orchid", [122, 92, 176], [76, 132, 112]),
    ("Rose", [188, 74, 108], [74, 124, 128]),
    ("Gold", [176, 124, 24], [62, 114, 158]),
    ("Graphite", [72, 86, 102], [108, 124, 112]),
];
impl Default for Appearance {
    fn default() -> Self {
        Self {
            dark: false,
            background: Background::Solid,
            color: [231, 239, 245],
            second: [198, 218, 231],
            texture: Texture::None,
            clock: CLOCK_COLORS[0].1,
            clock_break: CLOCK_COLORS[0].2,
        }
    }
}
impl Appearance {
    pub fn ink(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(230, 239, 246)
        } else {
            Color32::from_rgb(36, 59, 77)
        }
    }
    pub fn quiet(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(171, 193, 210)
        } else {
            Color32::from_rgb(92, 116, 133)
        }
    }
    pub fn paper(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(30, 44, 57)
        } else {
            Color32::from_rgb(248, 251, 253)
        }
    }
    /// Progress and primary-button colour for the current phase.
    pub fn accent(&self, on_break: bool) -> Color32 {
        let [r, g, b] = if on_break {
            self.clock_break
        } else {
            self.clock
        };
        Color32::from_rgb(r, g, b)
    }
    /// Text colour that stays readable on top of a clock colour.
    pub fn on_accent(&self, fill: Color32) -> Color32 {
        let linear = |v: u8| {
            let c = v as f32 / 255.;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let lum = linear(fill.r()) * 0.2126 + linear(fill.g()) * 0.7152 + linear(fill.b()) * 0.0722;
        // Pick whichever of white or near-black has the higher contrast.
        if 1.05 / (lum + 0.05) >= (lum + 0.05) / 0.061 {
            Color32::WHITE
        } else {
            Color32::from_rgb(20, 28, 36)
        }
    }
    /// Unfilled part of the progress ring.
    pub fn track(&self) -> Color32 {
        if self.dark {
            Color32::from_white_alpha(28)
        } else {
            Color32::from_black_alpha(26)
        }
    }
    /// Shared widget styling so unchecked boxes, sliders and disabled buttons stay visible.
    pub fn visuals(&self) -> egui::Visuals {
        let mut visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.override_text_color = Some(self.ink());
        let accent = self.accent(false);
        visuals.widgets.active.bg_fill = accent;
        visuals.selection.bg_fill = accent;
        let (fill, edge) = if self.dark {
            (Color32::from_white_alpha(18), Color32::from_white_alpha(80))
        } else {
            (Color32::from_black_alpha(14), Color32::from_black_alpha(70))
        };
        visuals.widgets.inactive.weak_bg_fill = fill;
        visuals.widgets.inactive.bg_fill = fill;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, edge);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, edge.gamma_multiply(0.6));
        let round = egui::CornerRadius::same(10);
        visuals.widgets.noninteractive.corner_radius = round;
        visuals.widgets.inactive.corner_radius = round;
        visuals.widgets.hovered.corner_radius = round;
        visuals.widgets.active.corner_radius = round;
        visuals.widgets.open.corner_radius = round;
        visuals.window_corner_radius = egui::CornerRadius::same(18);
        visuals.menu_corner_radius = egui::CornerRadius::same(12);
        visuals.handle_shape = egui::style::HandleShape::Circle;
        visuals.slider_trailing_fill = true;
        visuals.extreme_bg_color = if self.dark {
            Color32::from_rgb(20, 31, 42)
        } else {
            Color32::WHITE
        };
        visuals
    }
    // Preserve the selected hue while ensuring readable text at both gradient ends.
    fn readable(&self, mut rgb: [u8; 3]) -> [u8; 3] {
        let luminance = |rgb: [u8; 3]| {
            let linear = rgb.map(|v| {
                let c = v as f32 / 255.;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            });
            linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
        };
        let text = self.quiet();
        let t = luminance([text.r(), text.g(), text.b()]);
        for _ in 0..255 {
            let b = luminance(rgb);
            let contrast = (t.max(b) + 0.05) / (t.min(b) + 0.05);
            if contrast >= 4.5 {
                break;
            }
            rgb = rgb.map(|v| {
                if self.dark {
                    v.saturating_sub(1)
                } else {
                    v.saturating_add(1)
                }
            });
        }
        rgb
    }
    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect) {
        let color = |rgb: [u8; 3]| Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        let first = color(self.readable(self.color));
        let second = if self.background == Background::Gradient {
            color(self.readable(self.second))
        } else {
            first
        };
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.left_top(), first);
        mesh.colored_vertex(rect.right_top(), first);
        mesh.colored_vertex(rect.right_bottom(), second);
        mesh.colored_vertex(rect.left_bottom(), second);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
        let tint = if self.dark {
            Color32::from_white_alpha(9)
        } else {
            Color32::from_black_alpha(9)
        };
        match self.texture {
            Texture::None => {}
            Texture::Dots => {
                for y in (0..rect.height() as usize).step_by(18) {
                    for x in (0..rect.width() as usize).step_by(18) {
                        painter.circle_filled(rect.min + egui::vec2(x as f32, y as f32), 1., tint);
                    }
                }
            }
            Texture::Linen => {
                for x in (0..rect.width() as usize).step_by(5) {
                    painter.line_segment(
                        [
                            Pos2::new(rect.left() + x as f32, rect.top()),
                            Pos2::new(rect.left() + x as f32, rect.bottom()),
                        ],
                        Stroke::new(0.5, tint),
                    );
                }
                for y in (0..rect.height() as usize).step_by(5) {
                    painter.line_segment(
                        [
                            Pos2::new(rect.left(), rect.top() + y as f32),
                            Pos2::new(rect.right(), rect.top() + y as f32),
                        ],
                        Stroke::new(0.5, tint),
                    );
                }
            }
            Texture::Grain => {
                for y in (0..rect.height() as usize).step_by(7) {
                    for x in (0..rect.width() as usize).step_by(7) {
                        let hash =
                            (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
                        let offset = egui::vec2((hash % 7) as f32, ((hash >> 8) % 7) as f32);
                        painter.circle_filled(
                            rect.min + egui::vec2(x as f32, y as f32) + offset,
                            0.65,
                            tint,
                        );
                    }
                }
            }
        }
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        if ui.checkbox(&mut self.dark, "Dark mode").changed() {
            self.color = if self.dark {
                [19, 30, 41]
            } else {
                [231, 239, 245]
            };
            self.second = if self.dark {
                [43, 62, 82]
            } else {
                [198, 218, 231]
            };
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.background, Background::Solid, "Solid color");
            ui.selectable_value(&mut self.background, Background::Gradient, "Gradient");
        });
        ui.horizontal(|ui| {
            ui.label("Background color");
            ui.color_edit_button_srgb(&mut self.color);
        });
        if self.background == Background::Gradient {
            ui.horizontal(|ui| {
                ui.label("Second color");
                ui.color_edit_button_srgb(&mut self.second);
            });
        }
        egui::ComboBox::from_label("Texture")
            .selected_text(format!("{:?}", self.texture))
            .show_ui(ui, |ui| {
                for texture in [Texture::None, Texture::Grain, Texture::Linen, Texture::Dots] {
                    ui.selectable_value(&mut self.texture, texture, format!("{texture:?}"));
                }
            });
        ui.add_space(4.0);
        ui.label("Clock color");
        ui.horizontal(|ui| {
            for (name, focus, rest) in CLOCK_COLORS {
                let selected = self.clock == focus && self.clock_break == rest;
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
                let fill = Color32::from_rgb(focus[0], focus[1], focus[2]);
                ui.painter().circle_filled(rect.center(), 11.0, fill);
                if selected {
                    ui.painter()
                        .circle_stroke(rect.center(), 13.5, Stroke::new(2.0, self.ink()));
                }
                if response.on_hover_text(name).clicked() {
                    self.clock = focus;
                    self.clock_break = rest;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Focus");
            ui.color_edit_button_srgb(&mut self.clock);
            ui.label("Break");
            ui.color_edit_button_srgb(&mut self.clock_break);
        });
        ui.label("Colors apply immediately and adjust gently for readable text.");
        if ui.button("Restore original appearance").clicked() {
            *self = Self::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_backgrounds_retain_contrast_at_both_gradient_ends() {
        for dark in [false, true] {
            let appearance = Appearance {
                dark,
                ..Default::default()
            };
            for color in [
                [0, 0, 0],
                [255, 255, 255],
                [255, 0, 0],
                [0, 255, 0],
                [0, 0, 255],
            ] {
                let safe = appearance.readable(color);
                let linear = |v: u8| {
                    let c = v as f32 / 255.;
                    if c <= 0.04045 {
                        c / 12.92
                    } else {
                        ((c + 0.055) / 1.055).powf(2.4)
                    }
                };
                let lum = |c: [u8; 3]| {
                    linear(c[0]) * 0.2126 + linear(c[1]) * 0.7152 + linear(c[2]) * 0.0722
                };
                let quiet = appearance.quiet();
                let a = lum(safe);
                let b = lum([quiet.r(), quiet.g(), quiet.b()]);
                assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
            }
        }
    }
    #[test]
    fn existing_preferences_keep_original_appearance() {
        let prefs: crate::soundscape::Preferences =
            serde_json::from_str(r#"{"version":1}"#).unwrap();
        assert_eq!(prefs.appearance, Appearance::default());
    }
    #[test]
    fn custom_appearance_survives_preference_roundtrip() {
        let mut prefs = crate::soundscape::Preferences::default();
        prefs.appearance = Appearance {
            dark: true,
            background: Background::Gradient,
            color: [12, 34, 56],
            second: [78, 90, 123],
            texture: Texture::Linen,
            clock: [200, 80, 40],
            clock_break: [10, 120, 130],
        };
        let restored: crate::soundscape::Preferences =
            serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        assert_eq!(prefs, restored);
    }
}
