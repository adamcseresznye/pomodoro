use crate::{
    app::App,
    config::Config,
    event::{fmt_mmss, fmt_total_minutes, PomodoroTask},
    key_handler::KeyAction,
    notification,
    soundscape::{
        self, Engine, Preferences, Settings as SoundSettings, DESCRIPTIONS, NAMES, PRESETS,
    },
};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use std::time::{Duration, Instant};

const BACKGROUND: Color32 = Color32::from_rgb(231, 239, 245);
const INK: Color32 = Color32::from_rgb(36, 59, 77);
const QUIET: Color32 = Color32::from_rgb(92, 116, 133);
const BLUE: Color32 = Color32::from_rgb(62, 114, 158);
const PAPER: Color32 = Color32::from_rgb(248, 251, 253);

pub fn run(cfg: Config) -> eframe::Result {
    let preferences = Preferences::load(&soundscape::preferences_path());
    let cfg = if std::env::args().len() > 1 {
        cfg
    } else {
        preferences.session.clone()
    };
    notification::set_muted(cfg.muted);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pomodoro • Soundscapes")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/app-icon.png"))
                    .expect("bundled application icon"),
            )
            .with_inner_size([1080.0, 800.0])
            .with_min_inner_size([640.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "pomodoro-soundscapes",
        options,
        Box::new(move |cc| Ok(Box::new(Desktop::new(&cc.egui_ctx, cfg, preferences)))),
    )
}

struct Desktop {
    timer: App,
    settings: Config,
    last_tick: Instant,
    sound: SoundSettings,
    audio: Engine,
    preview: Option<(Option<usize>, Instant)>,
    preview_saved_enabled: bool,
    save_at: Option<Instant>,
    save_error: Option<String>,
}

impl Desktop {
    fn new(ctx: &egui::Context, cfg: Config, prefs: Preferences) -> Self {
        ctx.set_theme(egui::Theme::Light);
        let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.override_text_color = Some(INK);
        style.visuals.panel_fill = BACKGROUND;
        style.visuals.widgets.active.bg_fill = BLUE;
        style.visuals.selection.bg_fill = BLUE;
        style.spacing.item_spacing = Vec2::new(12.0, 10.0);
        style.spacing.button_padding = Vec2::new(16.0, 10.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(16.0));
        ctx.set_global_style(style);
        Self {
            settings: cfg.clone(),
            timer: App::new(cfg),
            last_tick: Instant::now(),
            sound: prefs.sound,
            audio: {
                #[cfg(test)]
                {
                    Engine::silent()
                }
                #[cfg(not(test))]
                {
                    Engine::new()
                }
            },
            preview: None,
            preview_saved_enabled: false,
            save_at: None,
            save_error: None,
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let bindings = [
            (egui::Key::Space, KeyAction::TogglePause),
            (egui::Key::Enter, KeyAction::Confirm),
            (egui::Key::P, KeyAction::Pause),
            (egui::Key::R, KeyAction::Resume),
            (egui::Key::S, KeyAction::Skip),
            (egui::Key::N, KeyAction::Skip),
            (egui::Key::X, KeyAction::Restart),
            (egui::Key::M, KeyAction::MuteToggle),
            (egui::Key::Q, KeyAction::Quit),
            (egui::Key::Escape, KeyAction::Quit),
        ];
        for (key, action) in bindings {
            if ctx.input(|i| i.key_pressed(key)) {
                self.timer.handle_action(action);
            }
        }
    }

    fn timer_panel(&mut self, ui: &mut egui::Ui) {
        let phase = match self.timer.phase() {
            PomodoroTask::Work => "Focus time",
            PomodoroTask::ShortBreak => "A little break",
            PomodoroTask::LongBreak => "Time to recharge",
        };
        let summary = self.timer.is_summary();
        let startup = self.timer.is_startup();
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(if summary { "Session complete" } else { phase }).size(26.0));
            ui.label(
                RichText::new(if summary {
                    "Your time, well spent."
                } else if startup {
                    "Choose your sound. Settle into focus."
                } else if self.timer.is_paused() {
                    "Paused. Take your time."
                } else {
                    "One thing at a time."
                })
                .color(QUIET),
            );
            ui.add_space(10.0);
            let side = ui.available_width().min(240.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::hover());
            let center = rect.center();
            let radius = side * 0.45;
            ui.painter().circle_stroke(
                center,
                radius,
                Stroke::new(6.0, Color32::from_rgb(204, 220, 232)),
            );
            let progress = if summary { 1.0 } else { self.timer.progress() };
            if progress > 0.0 {
                let points: Vec<_> = (0..=100)
                    .map(|i| {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + std::f32::consts::TAU * progress * i as f32 / 100.0;
                        center + Vec2::new(angle.cos(), angle.sin()) * radius
                    })
                    .collect();
                ui.painter()
                    .add(egui::Shape::line(points, Stroke::new(6.0, BLUE)));
            }
            ui.painter().text(
                center - Vec2::new(0.0, 6.0),
                egui::Align2::CENTER_CENTER,
                if summary {
                    format!("{}", self.timer.stats().work_completed)
                } else {
                    fmt_mmss(self.timer.remaining_secs())
                },
                FontId::monospace((side * 0.23).min(56.0)),
                INK,
            );
            ui.painter().text(
                center + Vec2::new(0.0, 38.0),
                egui::Align2::CENTER_CENTER,
                if summary {
                    "sessions finished"
                } else {
                    "remaining"
                },
                FontId::proportional(14.0),
                QUIET,
            );
            ui.add_space(8.0);
            let primary = if summary {
                "New session"
            } else if startup {
                "Start focusing"
            } else if self.timer.transition_message().is_some() {
                "Continue"
            } else if self.timer.is_paused() {
                "Resume"
            } else {
                "Pause"
            };
            if ui
                .add_sized(
                    [180.0, 44.0],
                    egui::Button::new(RichText::new(primary).color(Color32::WHITE)).fill(BLUE),
                )
                .clicked()
            {
                if summary {
                    self.settings.muted = notification::is_muted();
                    self.timer = App::new(self.settings.clone());
                } else {
                    if startup {
                        self.settings.muted = notification::is_muted();
                        self.timer.configure(self.settings.clone());
                    }
                    self.timer.handle_action(KeyAction::Confirm);
                }
            }
            if !startup && !summary {
                let labels = ["Skip", "Restart phase", "End session"];
                let font = egui::TextStyle::Button.resolve(ui.style());
                let widths = labels.map(|label| {
                    ui.painter()
                        .layout_no_wrap(label.into(), font.clone(), INK)
                        .size()
                        .x
                        + ui.spacing().button_padding.x * 2.0
                });
                let row_width = widths.iter().sum::<f32>() + ui.spacing().item_spacing.x * 2.0;
                let inset = ((ui.available_width() - row_width) * 0.5).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(inset);
                    if ui
                        .add_sized([widths[0], 38.0], egui::Button::new(labels[0]))
                        .clicked()
                    {
                        self.timer.handle_action(KeyAction::Skip);
                    }
                    if ui
                        .add_sized([widths[1], 38.0], egui::Button::new(labels[1]))
                        .clicked()
                    {
                        self.timer.handle_action(KeyAction::Restart);
                    }
                    if ui
                        .add_sized([widths[2], 38.0], egui::Button::new(labels[2]))
                        .clicked()
                    {
                        self.timer.handle_action(KeyAction::Quit);
                    }
                });
            }
            if let Some(message) = self.timer.transition_message() {
                ui.label(message);
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!(
                    "Session {} of {}",
                    (self.timer.stats().work_completed + 1).min(self.timer.config().cycles),
                    self.timer.config().cycles
                ))
                .color(QUIET),
            );
            ui.label(format!(
                "{} focus completed",
                fmt_total_minutes(self.timer.stats().focus_secs)
            ));
            ui.label(format!(
                "{} break completed",
                fmt_total_minutes(self.timer.stats().break_secs)
            ));
        });
        ui.add_space(12.0);
        egui::CollapsingHeader::new("Session settings")
            .default_open(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if !startup && !summary {
                    ui.label(
                        RichText::new("Changes apply to your next session.")
                            .size(13.0)
                            .color(QUIET),
                    );
                }
                ui.add(
                    egui::Slider::new(&mut self.settings.work_mins, 1..=180).text("Focus minutes"),
                );
                ui.add(
                    egui::Slider::new(&mut self.settings.short_mins, 1..=180).text("Short break"),
                );
                ui.add(egui::Slider::new(&mut self.settings.long_mins, 1..=180).text("Long break"));
                ui.add(egui::Slider::new(&mut self.settings.cycles, 1..=12).text("Sessions"));
                ui.checkbox(&mut self.settings.auto_start, "Start phases automatically");
                ui.checkbox(&mut self.settings.desktop, "Desktop notifications");
                // Only startup configuration changes the current timer.
                self.settings.muted = notification::is_muted();
                self.timer.configure(self.settings.clone());
            });
    }

    fn sound_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
        ui.spacing_mut().button_padding = Vec2::new(8.0, 3.0);
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(14.0));
        let audio_ready = self.audio.message().starts_with("Ready");
        ui.label(RichText::new("Your soundscape").size(26.0));
        ui.label(RichText::new("Start with a preset. Make it your own.").color(QUIET));
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            for (index, (name, _)) in PRESETS.iter().enumerate() {
                if ui
                    .selectable_label(self.sound.preset == Some(index), *name)
                    .clicked()
                {
                    self.sound.choose(index);
                    if self.preview.is_some() {
                        self.start_preview(None);
                    }
                }
            }
        });
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(PAPER)
            .corner_radius(12.0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut self.sound.enabled, "Soundscape on")
                        .changed()
                        && !self.sound.enabled
                    {
                        self.preview = None;
                    }
                    if ui
                        .add_enabled(
                            audio_ready,
                            egui::Button::new(if self.preview.is_some() {
                                "Stop preview"
                            } else {
                                "Preview mix"
                            }),
                        )
                        .clicked()
                    {
                        if self.preview.is_some() {
                            self.stop_preview();
                        } else {
                            self.start_preview(None);
                        }
                    }
                    if ui.small_button("Clear mix").clicked() {
                        self.sound.levels = [0.; soundscape::COUNT];
                        self.sound.preset = None;
                        self.stop_preview();
                    }
                });
                ui.add(
                    egui::Slider::new(&mut self.sound.master, 0.0..=1.0)
                        .text("Master volume")
                        .custom_formatter(percent),
                );
                ui.label(
                    RichText::new(if self.preview.is_some() {
                        "Previewing for 8 seconds"
                    } else if self.playing() {
                        "Playing"
                    } else {
                        "Ready for your next focus session"
                    })
                    .size(13.0)
                    .color(QUIET),
                );
                ui.add_space(6.0);
                for index in 0..soundscape::COUNT {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(NAMES[index]).strong())
                            .on_hover_text(DESCRIPTIONS[index]);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(audio_ready, egui::Button::new("Preview").small())
                                .on_hover_text("Listen to this layer for 8 seconds")
                                .clicked()
                            {
                                self.start_preview(Some(index));
                            }
                            if ui
                                .add(
                                    egui::Slider::new(&mut self.sound.levels[index], 0.0..=1.0)
                                        .custom_formatter(percent),
                                )
                                .changed()
                            {
                                self.sound.preset = None;
                            }
                        });
                    });
                }
            });
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.sound.during_breaks,
            "Continue soundscape during breaks",
        );
        let mut bell = !notification::is_muted();
        if ui
            .checkbox(&mut bell, "Play a bell when a phase ends")
            .changed()
        {
            notification::set_muted(!bell);
        }
        ui.label(
            RichText::new("Alpha tones: use stereo headphones for the two separate tones.")
                .size(12.0)
                .color(QUIET),
        );
        let status = self.audio.message();
        ui.label(RichText::new(status).size(12.0).color(QUIET));
        if let Some(error) = &self.save_error {
            ui.colored_label(Color32::DARK_RED, error);
        }
        egui::CollapsingHeader::new("Audio credits").show(ui, |ui| {
            ui.label("Rain, thunder, birds and ocean: BigSoundBank, CC0. Edited, level matched, and looped.");
            ui.hyperlink_to("BigSoundBank sources", "https://bigsoundbank.com/");
            ui.label("Rainforest: Jungle Sound Thailand Phuket by Amada44. Resampled, level matched, and looped under CC BY-SA 3.0.");
            ui.hyperlink_to("Original rainforest recording", "https://commons.wikimedia.org/wiki/File:Jungle_Sound_Thailand_Phuket.flac");
            ui.hyperlink_to("CC BY-SA 3.0 license", "https://creativecommons.org/licenses/by-sa/3.0/");
            ui.label("Pink noise, brown noise and alpha tones are generated by this app.");
        });
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let before = (
            self.sound.clone(),
            self.settings.clone(),
            notification::is_muted(),
        );
        self.shortcuts(&ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(24.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Pomodoro").size(30.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(if notification::is_muted() {
                                "Bell off"
                            } else {
                                "Bell on"
                            })
                            .clicked()
                        {
                            notification::toggle_muted();
                        }
                    });
                });
                ui.add_space(14.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if ui.available_width() >= 850.0 {
                        ui.columns(2, |columns| {
                            columns[0].set_max_width(380.0);
                            self.timer_panel(&mut columns[0]);
                            self.sound_panel(&mut columns[1]);
                        });
                    } else {
                        self.timer_panel(ui);
                        ui.add_space(24.0);
                        self.sound_panel(ui);
                    }
                    ui.add_space(18.0);
                    ui.separator();
                    ui.label(
                        RichText::new("Space: pause / resume    S: skip    X: restart    M: bell")
                            .size(12.0)
                            .color(QUIET),
                    );
                });
            });
        self.sync_audio();
        if before
            != (
                self.sound.clone(),
                self.settings.clone(),
                notification::is_muted(),
            )
        {
            self.save_at = Some(Instant::now() + Duration::from_millis(700));
        }
    }

    fn timer_active(&self) -> bool {
        !self.timer.is_startup() && !self.timer.is_summary() && !self.timer.done
    }
    fn playing(&self) -> bool {
        self.preview.is_some()
            || (self.sound.enabled
                && soundscape::should_play(
                    self.timer_active(),
                    self.timer.is_paused(),
                    self.timer.phase(),
                    self.sound.during_breaks,
                ))
    }
    fn start_preview(&mut self, layer: Option<usize>) {
        if self.preview.is_none() {
            self.preview_saved_enabled = self.sound.enabled;
        }
        self.preview = Some((layer, Instant::now() + Duration::from_secs(8)));
        self.audio.restart(layer);
    }
    fn stop_preview(&mut self) {
        if self.preview.take().is_some() {
            self.sound.enabled = self.preview_saved_enabled;
        }
    }
    fn sync_audio(&mut self) {
        if self
            .preview
            .is_some_and(|(_, deadline)| Instant::now() >= deadline)
        {
            self.stop_preview();
        }
        let active = soundscape::should_play(
            self.timer_active(),
            self.timer.is_paused(),
            self.timer.phase(),
            self.sound.during_breaks,
        );
        let gains = match self.preview {
            Some((Some(layer), _)) => self.sound.gains(false, Some(layer)),
            Some((None, _)) => {
                let mut sound = self.sound.clone();
                sound.enabled = true;
                sound.gains(true, None)
            }
            None => self.sound.gains(active, None),
        };
        self.audio.update(gains);
    }
    fn save_preferences(&mut self) {
        let mut session = self.settings.clone();
        session.muted = notification::is_muted();
        let prefs = Preferences {
            version: 1,
            session,
            sound: self.sound.clone(),
        };
        self.save_error = prefs
            .save(&soundscape::preferences_path())
            .err()
            .map(|e| format!("Could not save preferences: {e}"));
        self.save_at = None;
    }
}

impl eframe::App for Desktop {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Wall-clock timing continues when the window is minimized.
        if self.last_tick.elapsed() >= Duration::from_millis(100) {
            self.timer.update();
            self.last_tick = Instant::now();
        }
        self.sync_audio();
        if self
            .save_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.save_preferences();
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        if self.timer.done {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_preferences();
    }
}

fn percent(value: f64, _: std::ops::RangeInclusive<usize>) -> String {
    format!("{:.0}%", value * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_controls_share_timer_state() {
        let ctx = egui::Context::default();
        let mut desktop = Desktop::new(&ctx, Config::default(), Preferences::default());
        desktop.settings.work_mins = 40;
        desktop.timer.configure(desktop.settings.clone());
        desktop.timer.handle_action(KeyAction::Confirm);
        assert_eq!(desktop.timer.remaining_secs(), 2400);
        desktop.timer.handle_action(KeyAction::TogglePause);
        assert!(desktop.timer.is_paused());
        desktop.timer.handle_action(KeyAction::TogglePause);
        assert!(!desktop.timer.is_paused());
        desktop.timer.handle_action(KeyAction::Quit);
        assert!(desktop.timer.is_summary());
    }

    #[test]
    fn mixer_renders_all_layers_at_desktop_and_compact_sizes() {
        for (width, height) in [(1080.0, 800.0), (640.0, 560.0)] {
            let ctx = egui::Context::default();
            let mut desktop = Desktop::new(&ctx, Config::default(), Preferences::default());
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, height),
                    )),
                    ..Default::default()
                },
                |ui| desktop.render(ui),
            );
            assert!(!output.shapes.is_empty());
            assert!(!desktop.timer_active());
            assert!(!desktop.playing());
        }
    }

    #[test]
    fn preview_restores_playback_and_all_presets_are_available() {
        let ctx = egui::Context::default();
        let mut desktop = Desktop::new(&ctx, Config::default(), Preferences::default());
        desktop.sound.enabled = false;
        desktop.start_preview(Some(7));
        assert!(desktop.playing());
        desktop.stop_preview();
        assert!(!desktop.sound.enabled);
        assert!(!desktop.playing());
        assert_eq!(PRESETS.len(), 8);
        for index in 0..8 {
            desktop.sound.choose(index);
            assert!(desktop.sound.levels.iter().any(|v| *v > 0.));
        }
    }
}
