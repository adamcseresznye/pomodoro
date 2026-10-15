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
            .with_inner_size([960.0, 700.0])
            .with_min_inner_size([560.0, 640.0]),
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
    confirm_end: bool,
    pending_delete: Option<usize>,
    soundscape_open: bool,
    settings_open: bool,
    custom_input: String,
    custom_note: String,
    loaded_custom: Option<String>,
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
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Manrope".into(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/Manrope-Regular.ttf"))
                .into(),
        );
        fonts.font_data.insert(
            "Manrope Tabular".into(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/Manrope-Tabular.ttf"))
                .into(),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Manrope".into());
        // egui doesn't apply OpenType's tnum feature; this instance maps the
        // countdown digits to Manrope's tabular glyphs before bundling.
        let mut timer_fonts = fonts.families[&egui::FontFamily::Proportional].clone();
        timer_fonts.insert(0, "Manrope Tabular".into());
        fonts
            .families
            .insert(egui::FontFamily::Name("timer".into()), timer_fonts);
        ctx.set_fonts(fonts);
        let theme = if prefs.appearance.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = prefs.appearance.visuals();
        style.visuals.panel_fill = BACKGROUND;
        style.spacing.item_spacing = Vec2::new(10.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(16.0));
        ctx.set_global_style(style);
        let desktop = Self {
            appearance: prefs.appearance,
            appearance_open: false,
            experience: prefs.experience,
            atmospheres_open: false,
            atmosphere_name: String::new(),
            confirm_end: false,
            pending_delete: None,
            soundscape_open: false,
            settings_open: false,
            custom_input: String::new(),
            custom_note: String::new(),
            loaded_custom: None,
            recorded_sessions: 0,
            recorded_seconds: 0,
            fade_from: [0.; soundscape::COUNT],
            fade_target: [0.; soundscape::COUNT],
            fade_started: Instant::now(),
            settings: cfg.clone(),
            timer: App::new_waiting(cfg),
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
        };
        desktop
    }

    /// Keep the engine's extra layer in step with `sound.custom_source`, whether it
    /// came from saved preferences, the input box or an applied atmosphere.
    fn sync_custom(&mut self) {
        use crate::custom_audio::{classify, label, Link};
        if self.sound.custom_source == self.loaded_custom {
            return;
        }
        match self.sound.custom_source.clone() {
            Some(source) => match classify(&source) {
                link @ (Link::Download(_) | Link::File(_)) => {
                    self.audio.load_custom(link, label(&source));
                }
                _ => self.sound.custom_source = None,
            },
            None => self.audio.clear_custom(),
        }
        self.loaded_custom = self.sound.custom_source.clone();
    }

    fn use_custom(&mut self, input: &str) {
        use crate::custom_audio::{classify, Link};
        self.custom_note.clear();
        match classify(input) {
            Link::Browser(url) => {
                self.custom_note = match webbrowser::open(&url) {
                    Ok(()) => "Opened in your browser. It plays outside this app, so the timer can't pause it.".into(),
                    Err(_) => "Couldn't open your browser.".into(),
                };
                self.custom_input.clear();
            }
            Link::Invalid => {
                self.custom_note =
                    "Paste an http(s) link to an audio file, or the path of a file.".into();
            }
            _ => {
                self.sound.custom_source = Some(input.trim().trim_matches('"').to_owned());
                self.custom_input.clear();
            }
        }
    }

    fn clear_custom(&mut self) {
        self.sound.custom_source = None;
        self.custom_note.clear();
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || self.soundscape_open || self.settings_open {
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
                // Ending a running session takes two presses so one stray key can't lose it.
                if action == KeyAction::Quit && self.timer_active() && !self.confirm_end {
                    self.confirm_end = true;
                    continue;
                }
                self.confirm_end = false;
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
        self.card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(if summary { "Session complete" } else { phase }).size(23.0),
                );
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
                ui.add_space(4.0);
                if startup && !self.experience.focus_view {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.experience.intention)
                            .hint_text("What are you focusing on?")
                            .desired_width(ui.available_width() * 0.9)
                            .char_limit(120),
                    );
                } else if !self.experience.intention.trim().is_empty() {
                    ui.label(RichText::new(&self.experience.intention).size(18.0));
                }
                // Rows that are present take room from the ring, so everything fits without scrolling.
                let mut height = ui.ctx().content_rect().height();
                if !startup && !summary && !self.experience.focus_view {
                    height -= 52.0; // Skip, restart and end session
                }
                if self.experience.focus_view {
                    height += 62.0; // No soundscape and settings buttons
                }
                if !startup && self.experience.intention.trim().is_empty() {
                    height += 32.0;
                }
                let side = timer_side(ui.available_width(), height);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::hover());
                let center = rect.center();
                let radius = side * 0.45;
                let accent = self
                    .appearance
                    .accent(self.timer.phase() != PomodoroTask::Work);
                ui.painter().circle_stroke(
                    center,
                    radius,
                    Stroke::new(10.0, self.appearance.track()),
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
                    let head = points[points.len() - 1];
                    let tail = points[0];
                    ui.painter()
                        .add(egui::Shape::line(points, Stroke::new(10.0, accent)));
                    // Round caps on both ends of the progress arc.
                    ui.painter().circle_filled(head, 5.0, accent);
                    ui.painter().circle_filled(tail, 5.0, accent);
                }
                ui.painter().text(
                    center - Vec2::new(0.0, 6.0),
                    egui::Align2::CENTER_CENTER,
                    if summary {
                        format!("{}", self.timer.stats().work_completed)
                    } else {
                        fmt_mmss(self.timer.remaining_secs())
                    },
                    FontId::new(side * 0.23, egui::FontFamily::Name("timer".into())),
                    self.appearance.ink(),
                );
                ui.painter().text(
                    center + Vec2::new(0.0, side * 0.18),
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
                        [ui.available_width() * 0.7, 50.0],
                        egui::Button::new(
                            RichText::new(primary)
                                .size(17.0)
                                .color(self.appearance.on_accent(accent)),
                        )
                        .fill(accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(25.0),
                    )
                    .clicked()
                {
                    if summary {
                        self.settings.muted = notification::is_muted();
                        self.record_progress();
                        self.recorded_sessions = 0;
                        self.recorded_seconds = 0;
                        self.timer = App::new_waiting(self.settings.clone());
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
                        let soft = |label: &'static str| {
                            egui::Button::new(label)
                                .fill(self.appearance.track())
                                .stroke(Stroke::NONE)
                                .corner_radius(19.0)
                        };
                        if ui.add_sized([widths[0], 38.0], soft(labels[0])).clicked() {
                            self.timer.handle_action(KeyAction::Skip);
                        }
                        if ui.add_sized([widths[1], 38.0], soft(labels[1])).clicked() {
                            self.timer.handle_action(KeyAction::Restart);
                        }
                        let end_label = if self.confirm_end {
                            "Click again to end"
                        } else {
                            labels[2]
                        };
                        let end = egui::Button::new(RichText::new(end_label).color(
                            if self.confirm_end {
                                Color32::from_rgb(190, 60, 50)
                            } else {
                                self.appearance.quiet()
                            },
                        ))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE);
                        if ui.add(end).clicked() {
                            if self.confirm_end {
                                self.confirm_end = false;
                                self.timer.handle_action(KeyAction::Quit);
                            } else {
                                self.confirm_end = true;
                            }
                        }
                    });
                }
                if let Some(message) = self.timer.transition_message() {
                    ui.label(message);
                }
                ui.add_space(8.0);
                // One pip per focus session: filled when done, outlined for the current one.
                let total = self.timer.config().cycles as usize;
                let done = self.timer.stats().work_completed as usize;
                let (pips, _) = ui.allocate_exact_size(
                    Vec2::new(total as f32 * 18.0 - 6.0, 12.0),
                    egui::Sense::hover(),
                );
                for i in 0..total {
                    let c = pips.left_center() + Vec2::new(6.0 + i as f32 * 18.0, 0.0);
                    if i < done {
                        ui.painter().circle_filled(c, 5.0, accent);
                    } else if i == done && !summary {
                        ui.painter().circle_stroke(c, 4.5, Stroke::new(2.0, accent));
                    } else {
                        ui.painter().circle_filled(c, 5.0, self.appearance.track());
                    }
                }
                let mut line = format!("Session {} of {}", (done + 1).min(total), total);
                if summary {
                    line = format!(
                        "{} focus · {} break",
                        fmt_total_minutes(self.timer.stats().focus_secs),
                        fmt_total_minutes(self.timer.stats().break_secs)
                    );
                }
                ui.label(
                    RichText::new(line)
                        .size(14.0)
                        .color(self.appearance.quiet()),
                );
                if !self.experience.focus_view {
                    ui.add_space(12.0);
                    let mix = if !self.sound.enabled {
                        "off"
                    } else {
                        self.sound.preset.map_or("your mix", |i| PRESETS[i].0)
                    };
                    let labels = [format!("Soundscape: {mix}"), "Settings".to_string()];
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    let total: f32 = labels
                        .iter()
                        .map(|l| {
                            ui.painter()
                                .layout_no_wrap(l.clone(), font.clone(), self.appearance.ink())
                                .size()
                                .x
                                + 26.0
                        })
                        .sum();
                    let inset = ((ui.available_width() - total - ui.spacing().item_spacing.x)
                        * 0.5)
                        .max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(inset);
                        if chip(ui, &labels[0], false, &self.appearance).clicked() {
                            self.soundscape_open = true;
                        }
                        if chip(ui, &labels[1], false, &self.appearance).clicked() {
                            self.settings_open = true;
                        }
                    });
                }
            });
        });
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        let startup = self.timer.is_startup();
        let summary = self.timer.is_summary();
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Session settings").size(23.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if chip(ui, "Done", true, &self.appearance).clicked() {
                    self.settings_open = false;
                }
            });
        });
        if !startup && !summary {
            ui.label(
                RichText::new("Changes apply to your next session.")
                    .size(13.0)
                    .color(self.appearance.quiet()),
            );
        }
        ui.add_space(4.0);
        egui::Grid::new("session_lengths")
            .num_columns(4)
            .spacing([14.0, 10.0])
            .show(ui, |ui| {
                ui.label("Focus");
                ui.add(
                    egui::DragValue::new(&mut self.settings.work_mins)
                        .range(1..=180)
                        .suffix(" min"),
                );
                ui.label("Sessions");
                ui.add(egui::DragValue::new(&mut self.settings.cycles).range(1..=12));
                ui.end_row();
                ui.label("Short break");
                ui.add(
                    egui::DragValue::new(&mut self.settings.short_mins)
                        .range(1..=180)
                        .suffix(" min"),
                );
                ui.label("Long break");
                ui.add(
                    egui::DragValue::new(&mut self.settings.long_mins)
                        .range(1..=180)
                        .suffix(" min"),
                );
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.checkbox(&mut self.settings.auto_start, "Start phases automatically");
        ui.checkbox(&mut self.settings.desktop, "Desktop notifications");
        // Only startup configuration changes the current timer.
        self.settings.muted = notification::is_muted();
        self.timer.configure(self.settings.clone());
    }

    fn card_frame(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.appearance.paper())
            .corner_radius(20.0)
            .inner_margin(20.0)
            .shadow(egui::Shadow {
                offset: [0, 6],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(if self.appearance.dark { 70 } else { 22 }),
            })
    }

    fn sound_panel(&mut self, ui: &mut egui::Ui) {
        let roomy = ui.available_width() > 700.0;
        let text_size = if roomy { 16.0 } else { 14.0 };
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
        ui.spacing_mut().button_padding = Vec2::new(8.0, 3.0);
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(text_size));
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(text_size));
        let audio_ready = self.audio.message().starts_with("Ready");
        ui.scope(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Your soundscape").size(23.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if chip(ui, "Done", true, &self.appearance).clicked() {
                    self.soundscape_open = false;
                }
            });
        });
        ui.label(
            RichText::new("Start with a preset. Make it your own.").color(self.appearance.quiet()),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for (index, (name, _)) in PRESETS.iter().enumerate() {
                if chip(
                    ui,
                    name,
                    self.sound.preset == Some(index),
                    &self.appearance,
                )
                .clicked()
                {
                    self.sound.choose(index);
                    if self.preview.is_some() {
                        self.start_preview(None);
                    }
                }
            }
        });
        ui.add_space(10.0);
        egui::Frame::new()
            .fill(self.appearance.track())
            .corner_radius(14.0)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
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
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.sound.master, 0.0..=1.0)
                            .show_value(false),
                    );
                    ui.label(RichText::new(percent_text(self.sound.master as f64)).strong());
                    ui.label("Master volume");
                });
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
                ui.separator();
                ui.spacing_mut().slider_width =
                    (ui.available_width() - if roomy { 290.0 } else { 240.0 }).max(100.0);
                let row_spacing = (ui.ctx().content_rect().height() - 680.0).max(0.0) * 0.025 + 2.0;
                egui::Grid::new("sound_layers")
                    .num_columns(4)
                    .spacing([12.0, row_spacing.min(18.0)])
                    .min_row_height(22.0)
                    .show(ui, |ui| {
                        for index in 0..soundscape::COUNT {
                            let label = RichText::new(NAMES[index]);
                            let label = if self.sound.levels[index] > 0. {
                                label.strong()
                            } else {
                                label.color(self.appearance.quiet())
                            };
                            ui.label(label).on_hover_text(DESCRIPTIONS[index]);
                            if ui
                                .add(
                                    egui::Slider::new(&mut self.sound.levels[index], 0.0..=1.0)
                                        .show_value(false),
                                )
                                .changed()
                            {
                                self.sound.preset = None;
                            }
                            let level = self.sound.levels[index];
                            let text = RichText::new(percent_text(level as f64));
                            ui.add_sized(
                                [40.0, 20.0],
                                egui::Label::new(if level > 0. {
                                    text.strong()
                                } else {
                                    text.color(self.appearance.quiet())
                                }),
                            );
                            if ui
                                .add_enabled(audio_ready, egui::Button::new("Preview").small())
                                .on_hover_text("Listen to this layer for 8 seconds")
                                .clicked()
                            {
                                self.start_preview(Some(index));
                            }
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(10.0);
        egui::Frame::new()
            .fill(self.appearance.track())
            .corner_radius(14.0)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Your own audio").strong());
                ui.label(
                    RichText::new(
                        "Paste a link to an MP3, OGG or WAV file, or drop a file here. \
                         YouTube and Spotify links open in your browser.",
                    )
                    .size(13.0)
                    .color(self.appearance.quiet()),
                );
                match self.sound.custom_source.clone() {
                    None => {
                        ui.horizontal(|ui| {
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.custom_input)
                                    .hint_text("https://… or a file path")
                                    .desired_width((ui.available_width() - 70.0).max(80.0)),
                            );
                            let enter = edit.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if (ui.button("Add").clicked() || enter)
                                && !self.custom_input.trim().is_empty()
                            {
                                let text = self.custom_input.clone();
                                self.use_custom(&text);
                            }
                        });
                    }
                    Some(source) => {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(crate::custom_audio::label(&source)).strong(),
                            );
                            if ui.small_button("Remove").clicked() {
                                self.clear_custom();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Slider::new(&mut self.sound.custom_level, 0.0..=1.0)
                                    .show_value(false),
                            );
                            ui.label(
                                RichText::new(percent_text(self.sound.custom_level as f64))
                                    .strong(),
                            );
                            ui.label("Volume");
                        });
                    }
                }
                let note = if self.custom_note.is_empty() {
                    self.audio.custom_message()
                } else {
                    self.custom_note.clone()
                };
                if !note.is_empty() {
                    ui.label(
                        RichText::new(note)
                            .size(13.0)
                            .color(self.appearance.quiet()),
                    );
                }
            });
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.sound.during_breaks,
            "Continue soundscape during breaks",
        );
        if self.sound.levels[7] > 0. {
            ui.label(
                RichText::new("Alpha tones: use stereo headphones for the two separate tones.")
                    .size(13.0)
                    .color(self.appearance.quiet()),
            );
        }
        if !audio_ready {
            ui.label(
                RichText::new(self.audio.message())
                    .size(13.0)
                    .color(self.appearance.quiet()),
            );
        }
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
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
        if let Some(path) = dropped {
            self.use_custom(&path.to_string_lossy());
        }
        let visuals = self.appearance.visuals();
        *ui.visuals_mut() = visuals.clone();
        ctx.set_visuals(visuals);
        egui::Panel::bottom("session_footer")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 8)))
            .show(ui, |ui| {
                self.appearance
                    .paint(ui.painter(), ui.max_rect().expand(16.0));
                ui.horizontal_wrapped(|ui| {
                    if !self.experience.focus_view {
                        let day = self
                            .experience
                            .days
                            .get(&crate::experience::today())
                            .cloned()
                            .unwrap_or_default();
                        ui.label(
                            RichText::new(format!(
                                "Today: {} sessions · {} focus minutes",
                                day.sessions,
                                day.focus_secs / 60
                            ))
                            .size(13.0),
                        );
                    }
                    ui.label(
                        RichText::new("Space: pause / resume    S: skip    X: restart    M: bell")
                            .size(12.0)
                            .color(self.appearance.quiet()),
                    );
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(16.0))
            .show(ui, |ui| {
                self.appearance
                    .paint(ui.painter(), ui.max_rect().expand(16.0));
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Pomodoro")
                            .size(20.0)
                            .color(self.appearance.quiet()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        // Right-to-left: view toggle, then personalisation, then the bell.
                        if chip(
                            ui,
                            if self.experience.focus_view {
                                "Show everything"
                            } else {
                                "Focus view"
                            },
                            self.experience.focus_view,
                            &self.appearance,
                        )
                        .clicked()
                        {
                            self.experience.focus_view = !self.experience.focus_view;
                        }
                        if !self.experience.focus_view {
                            ui.add_space(8.0);
                            if chip(ui, "Appearance", self.appearance_open, &self.appearance)
                                .clicked()
                            {
                                self.appearance_open = !self.appearance_open;
                            }
                            if chip(ui, "Atmospheres", self.atmospheres_open, &self.appearance)
                                .clicked()
                            {
                                self.atmospheres_open = !self.atmospheres_open;
                            }
                            ui.add_space(8.0);
                        }
                        let bell = !notification::is_muted();
                        if chip(
                            ui,
                            if bell { "Bell on" } else { "Bell off" },
                            bell,
                            &self.appearance,
                        )
                        .on_hover_text("Play a bell when a phase ends (M)")
                        .clicked()
                        {
                            notification::toggle_muted();
                        }
                    });
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // The clock is the whole app: one centered column.
                    let width = ui.available_width().min(560.0);
                    ui.vertical_centered(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                if self.experience.focus_view {
                                    ui.add_space(12.0);
                                }
                                self.timer_panel(ui);
                            },
                        );
                    });
                });
            });
        if self.soundscape_open {
            let height = (ctx.content_rect().height() - 120.0).max(240.0);
            let width = (ctx.content_rect().width() - 100.0).clamp(280.0, 640.0);
            let modal = egui::Modal::new(egui::Id::new("soundscape_modal"))
                .frame(self.card_frame())
                .show(&ctx, |ui| {
                    ui.set_width(width);
                    ui.set_min_height(height);
                    egui::ScrollArea::vertical()
                        .max_height(height)
                        .show(ui, |ui| self.sound_panel(ui));
                });
            if modal.should_close() {
                self.soundscape_open = false;
            }
        }
        if self.settings_open {
            let modal = egui::Modal::new(egui::Id::new("settings_modal"))
                .frame(self.card_frame())
                .show(&ctx, |ui| {
                    ui.set_width((ctx.content_rect().width() - 100.0).clamp(280.0, 440.0));
                    self.settings_panel(ui);
                });
            if modal.should_close() {
                self.settings_open = false;
            }
        }
        egui::Window::new("Appearance")
            .anchor(egui::Align2::RIGHT_TOP, [-16.0, 64.0])
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
        self.sync_custom();
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
        let custom = match self.preview {
            Some((Some(_), _)) => 0.,
            Some((None, _)) => self.sound.custom_level * self.sound.master,
            None => self.sound.custom_gain(active),
        };
        self.audio.set_custom_gain(custom);
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
            .anchor(egui::Align2::RIGHT_TOP, [-16.0, 64.0])
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
                                if self.pending_delete == Some(index) {
                                    ui.label(format!("Delete “{}”?", atmosphere.name));
                                    if ui.small_button("Delete").clicked() {
                                        remove = Some(index);
                                    }
                                    if ui.small_button("Keep").clicked() {
                                        self.pending_delete = None;
                                    }
                                } else {
                                    if ui.button(&atmosphere.name).clicked() {
                                        self.appearance = atmosphere.appearance.clone();
                                        self.sound = atmosphere.sound.clone();
                                        self.preview = None;
                                    }
                                    if ui.small_button("Delete…").clicked() {
                                        self.pending_delete = Some(index);
                                    }
                                }
                            });
                        }
                    });
                if let Some(index) = remove {
                    self.experience.atmospheres.remove(index);
                    self.pending_delete = None;
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

fn timer_side(width: f32, height: f32) -> f32 {
    // Grow with both dimensions while retaining room for controls and settings.
    (width * 0.8).min((height - 450.0).max(150.0)).min(560.0)
}

fn percent_text(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

/// Rounded pill used for presets and header toggles.
fn chip(
    ui: &mut egui::Ui,
    text: &str,
    selected: bool,
    appearance: &crate::appearance::Appearance,
) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let color = if selected {
        appearance.on_accent(appearance.accent(false))
    } else {
        appearance.ink()
    };
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    let size = galley.size() + Vec2::new(26.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if selected {
        appearance.accent(false)
    } else if response.hovered() {
        appearance.track().gamma_multiply(1.8)
    } else {
        appearance.track()
    };
    ui.painter().rect_filled(rect, size.y / 2.0, fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_waits_for_start_with_saved_automatic_transitions() {
        let ctx = egui::Context::default();
        let cfg = Config {
            auto_start: true,
            ..Config::default()
        };
        let prefs = Preferences {
            session: cfg.clone(),
            ..Preferences::default()
        };
        let mut desktop = Desktop::new(&ctx, cfg, prefs);
        desktop.timer.update();
        assert!(desktop.timer.is_startup());
        assert!(!desktop.timer_active());
        assert!(!desktop.playing());
        assert_eq!(desktop.timer.remaining_secs(), 25 * 60);
        assert!(desktop.timer.config().auto_start);
        desktop.timer.handle_action(KeyAction::Confirm);
        assert!(desktop.timer_active());
        assert!(desktop.playing());
    }

    #[test]
    fn timer_scales_with_window_without_exceeding_its_column() {
        let compact = timer_side(350.0, 700.0);
        let maximized = timer_side(750.0, 1080.0);
        assert!(maximized > compact * 2.0);
        assert!(timer_side(200.0, 700.0) <= 200.0);
        assert!(timer_side(750.0, 600.0) < compact);
    }

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
        for width in [640., 920., 1080., 1920.] {
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
    fn soundscape_sheet_and_clock_colors_render() {
        for (width, height) in [(640., 560.), (960., 700.), (1920., 1080.)] {
            let ctx = egui::Context::default();
            let mut desktop = Desktop::new(&ctx, Config::default(), Preferences::default());
            desktop.soundscape_open = true;
            desktop.appearance.clock = [250, 220, 40];
            desktop.appearance_open = true;
            for _ in 0..2 {
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
            }
        }
        // Light text on dark colors, dark text on light ones.
        let appearance = crate::appearance::Appearance::default();
        assert_eq!(
            appearance.on_accent(Color32::from_rgb(62, 114, 158)),
            Color32::WHITE
        );
        assert_ne!(
            appearance.on_accent(Color32::from_rgb(250, 220, 40)),
            Color32::WHITE
        );
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
        for (width, height, dark) in [
            (920.0, 700.0, false),
            (1080.0, 800.0, false),
            (640.0, 560.0, true),
        ] {
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
