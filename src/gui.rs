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
const BLUE: Color32 = Color32::from_rgb(62, 114, 158);

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
    appearance: crate::appearance::Appearance,
    appearance_open: bool,
    experience: crate::experience::Experience,
    atmospheres_open: bool,
    atmosphere_name: String,
    recorded_sessions: u64,
    recorded_seconds: u64,
    fade_from: [f32; soundscape::COUNT],
    fade_target: [f32; soundscape::COUNT],
    fade_started: Instant,
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
        let theme = if prefs.appearance.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = if prefs.appearance.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.override_text_color = Some(prefs.appearance.ink());
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
            appearance: prefs.appearance,
            appearance_open: false,
            experience: prefs.experience,
            atmospheres_open: false,
            atmosphere_name: String::new(),
            recorded_sessions: 0,
            recorded_seconds: 0,
            fade_from: [0.; soundscape::COUNT],
            fade_target: [0.; soundscape::COUNT],
            fade_started: Instant::now(),
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
                .color(self.appearance.quiet()),
            );
            ui.add_space(10.0);
            if startup && !self.experience.focus_view {
                ui.add(
                    egui::TextEdit::singleline(&mut self.experience.intention)
                        .hint_text("What are you focusing on?")
                        .desired_width(300.0)
                        .char_limit(120),
                );
            } else if !self.experience.intention.trim().is_empty() {
                ui.label(RichText::new(&self.experience.intention).size(18.0));
            }
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
                self.appearance.ink(),
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
                self.appearance.quiet(),
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
                    self.record_progress();
                    self.recorded_sessions = 0;
                    self.recorded_seconds = 0;
                    self.timer = App::new(self.settings.clone());
                } else {
                    if startup {
                        self.settings.muted = notification::is_muted();
                        self.timer.configure(self.settings.clone());
                    }
                    self.timer.handle_action(KeyAction::Confirm);
                }
            }
            if !startup && !summary && !self.experience.focus_view {
                let labels = ["Skip", "Restart phase", "End session"];
                let font = egui::TextStyle::Button.resolve(ui.style());
                let widths = labels.map(|label| {
                    ui.painter()
                        .layout_no_wrap(label.into(), font.clone(), self.appearance.ink())
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
                .color(self.appearance.quiet()),
            );
            if !self.experience.focus_view || summary {
                ui.label(format!(
                    "{} focus completed",
                    fmt_total_minutes(self.timer.stats().focus_secs)
                ));
                ui.label(format!(
                    "{} break completed",
                    fmt_total_minutes(self.timer.stats().break_secs)
                ));
            }
        });
        if self.experience.focus_view {
            return;
        }
        ui.add_space(12.0);
        egui::CollapsingHeader::new("Session settings")
            .default_open(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if !startup && !summary && !self.experience.focus_view {
                    ui.label(
                        RichText::new("Changes apply to your next session.")
                            .size(13.0)
                            .color(self.appearance.quiet()),
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
        ui.label(
            RichText::new("Start with a preset. Make it your own.").color(self.appearance.quiet()),
        );
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
            .fill(self.appearance.paper())
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
                    .color(self.appearance.quiet()),
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
                .color(self.appearance.quiet()),
        );
        let status = self.audio.message();
        ui.label(
            RichText::new(status)
                .size(12.0)
                .color(self.appearance.quiet()),
        );
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
            self.experience.clone(),
            self.appearance.clone(),
            self.sound.clone(),
            self.settings.clone(),
            notification::is_muted(),
        );
        self.shortcuts(&ctx);
        let mut visuals = if self.appearance.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.override_text_color = Some(self.appearance.ink());
        visuals.widgets.active.bg_fill = BLUE;
        visuals.selection.bg_fill = BLUE;
        *ui.visuals_mut() = visuals.clone();
        ctx.set_visuals(visuals);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(24.0))
            .show(ui, |ui| {
                self.appearance
                    .paint(ui.painter(), ui.max_rect().expand(24.0));
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(8.0, 8.0);
                    ui.label(RichText::new("Pomodoro").size(30.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(if self.experience.focus_view {
                                "Show everything"
                            } else {
                                "Focus view"
                            })
                            .clicked()
                        {
                            self.experience.focus_view = !self.experience.focus_view;
                        }
                        if !self.experience.focus_view && ui.button("Atmospheres").clicked() {
                            self.atmospheres_open = !self.atmospheres_open;
                        }
                        if !self.experience.focus_view && ui.button("Appearance").clicked() {
                            self.appearance_open = !self.appearance_open;
                        }
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
                    if self.experience.focus_view {
                        ui.add_space(36.0);
                        self.timer_panel(ui);
                    } else if ui.available_width() >= 850.0 {
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
                    if !self.experience.focus_view {
                        let day = self
                            .experience
                            .days
                            .get(&crate::experience::today())
                            .cloned()
                            .unwrap_or_default();
                        ui.label(format!(
                            "Today: {} sessions completed | {} completed focus minutes",
                            day.sessions,
                            day.focus_secs / 60
                        ));
                    }
                    ui.label(
                        RichText::new("Space: pause / resume    S: skip    X: restart    M: bell")
                            .size(12.0)
                            .color(self.appearance.quiet()),
                    );
                });
            });
        egui::Window::new("Appearance")
            .open(&mut self.appearance_open)
            .resizable(false)
            .show(&ctx, |ui| self.appearance.controls(ui));
        self.atmospheres_window(&ctx);
        self.record_progress();
        self.sync_audio();
        if before
            != (
                self.experience.clone(),
                self.appearance.clone(),
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
        let now = Instant::now();
        let current = fade_values(
            self.fade_from,
            self.fade_target,
            now.duration_since(self.fade_started).as_secs_f32(),
        );
        if gains != self.fade_target {
            self.fade_from = current;
            self.fade_target = gains;
            self.fade_started = now;
        }
        self.audio.update(current);
    }
    fn record_progress(&mut self) {
        let stats = self.timer.stats();
        let sessions = stats.work_completed.saturating_sub(self.recorded_sessions);
        let seconds = stats.focus_secs.saturating_sub(self.recorded_seconds);
        self.recorded_sessions = stats.work_completed;
        self.recorded_seconds = stats.focus_secs;
        if sessions > 0 || seconds > 0 {
            self.experience
                .record(crate::experience::today(), sessions, seconds);
            self.save_at = Some(Instant::now() + Duration::from_millis(700));
        }
    }
    fn atmospheres_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Atmospheres")
            .open(&mut self.atmospheres_open)
            .default_width(360.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Your background and sound mix, together.");
                let mut remove = None;
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (index, atmosphere) in self.experience.atmospheres.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button(&atmosphere.name).clicked() {
                                    self.appearance = atmosphere.appearance.clone();
                                    self.sound = atmosphere.sound.clone();
                                    self.preview = None;
                                }
                                if ui.small_button("Delete").clicked() {
                                    remove = Some(index);
                                }
                            });
                        }
                    });
                if let Some(index) = remove {
                    self.experience.atmospheres.remove(index);
                }
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.atmosphere_name)
                        .hint_text("Name your atmosphere")
                        .char_limit(60),
                );
                let name = self.atmosphere_name.trim().to_owned();
                let exists = self.experience.atmospheres.iter().any(|a| a.name == name);
                if ui
                    .add_enabled(
                        !name.is_empty(),
                        egui::Button::new(if exists {
                            "Update saved atmosphere"
                        } else {
                            "Save current atmosphere"
                        }),
                    )
                    .clicked()
                {
                    let atmosphere = crate::experience::Atmosphere {
                        name: name.clone(),
                        appearance: self.appearance.clone(),
                        sound: self.sound.clone(),
                    };
                    if let Some(existing) = self
                        .experience
                        .atmospheres
                        .iter_mut()
                        .find(|a| a.name == name)
                    {
                        *existing = atmosphere;
                    } else {
                        self.experience.atmospheres.push(atmosphere);
                    }
                    self.atmosphere_name.clear();
                }
            });
    }
    fn save_preferences(&mut self) {
        self.record_progress();
        let mut session = self.settings.clone();
        session.muted = notification::is_muted();
        let prefs = Preferences {
            version: 1,
            session,
            sound: self.sound.clone(),
            appearance: self.appearance.clone(),
            experience: self.experience.clone(),
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
        self.record_progress();
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

fn fade_values(
    from: [f32; soundscape::COUNT],
    target: [f32; soundscape::COUNT],
    elapsed: f32,
) -> [f32; soundscape::COUNT] {
    let t = (elapsed / 2.0).clamp(0., 1.);
    let t = t * t * (3. - 2. * t);
    std::array::from_fn(|i| from[i] + (target[i] - from[i]) * t)
}

fn percent(value: f64, _: std::ops::RangeInclusive<usize>) -> String {
    format!("{:.0}%", value * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_fades_are_bounded_and_can_reverse_without_jumping() {
        let zero = [0.; soundscape::COUNT];
        let full = [1.; soundscape::COUNT];
        assert_eq!(fade_values(zero, full, 0.), zero);
        assert_eq!(fade_values(zero, full, 2.), full);
        let halfway = fade_values(zero, full, 1.);
        assert_eq!(halfway, [0.5; soundscape::COUNT]);
        assert_eq!(fade_values(halfway, zero, 0.), halfway);
        assert_eq!(fade_values(halfway, zero, 2.), zero);
    }
    #[test]
    fn focus_view_and_atmospheres_render_at_both_sizes() {
        for width in [640., 1080.] {
            let ctx = egui::Context::default();
            let mut desktop = Desktop::new(&ctx, Config::default(), Preferences::default());
            desktop.experience.focus_view = true;
            desktop.experience.intention = "Write the next paragraph".into();
            desktop.atmospheres_open = true;
            for _ in 0..2 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(width, 800.),
                        )),
                        ..Default::default()
                    },
                    |ui| desktop.render(ui),
                );
                assert!(!output.shapes.is_empty());
            }
        }
    }
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
        for (width, height, dark) in [(1080.0, 800.0, false), (640.0, 560.0, true)] {
            let ctx = egui::Context::default();
            let mut desktop = Desktop::new(&ctx, Config::default(), Preferences::default());
            desktop.appearance.dark = dark;
            desktop.appearance.background = crate::appearance::Background::Gradient;
            desktop.appearance.texture = crate::appearance::Texture::Grain;
            desktop.appearance_open = true;
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
