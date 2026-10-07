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
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            dark: false,
            background: Background::Solid,
            color: [231, 239, 245],
            second: [198, 218, 231],
            texture: Texture::None,
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
        };
        let restored: crate::soundscape::Preferences =
            serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        assert_eq!(prefs, restored);
    }
}
