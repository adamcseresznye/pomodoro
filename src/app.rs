use crate::config::Config;
use crate::event::{
    big_time_lines, change_state, cycle_tracker, fmt_minutes, fmt_mmss, fmt_total_minutes,
    next_hint, PomodoroTask, SessionStats,
};
use crate::notification;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Wrap},
    Frame,
};
use std::time::{Duration, Instant};

const TICK_MS: u64 = 100;

pub fn tick_duration() -> Duration {
    Duration::from_millis(TICK_MS)
}

enum Screen {
    Startup,
    Running,
    Transition { message: String, deadline: Instant },
    Summary { quit_early: bool },
}

pub struct App {
    cfg: Config,
    stats: SessionStats,
    task: PomodoroTask,
    pending: Option<PomodoroTask>,
    phase_total: u64,
    phase_start: Instant,
    paused_total: Duration,
    pause_started: Option<Instant>,
    paused: bool,
    remaining: u64,
    tick: u64,
    screen: Screen,
    /// True when the TUI should tear down (quit or session over + confirmed).
    pub done: bool,
    pub quit_early: bool,
}

impl App {
    pub fn new(cfg: Config) -> Self {
        let task = PomodoroTask::Work;
        let phase_total = task.duration(&cfg);
        let auto = cfg.auto_start;
        Self {
            cfg,
            stats: SessionStats::new(),
            task,
            pending: None,
            phase_total,
            phase_start: Instant::now(),
            paused_total: Duration::ZERO,
            pause_started: None,
            paused: false,
            remaining: phase_total,
            tick: 0,
            screen: if auto {
                Screen::Running
            } else {
                Screen::Startup
            },
            done: false,
            quit_early: false,
        }
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Desktop sessions wait for an explicit start, even with automatic transitions.
    pub fn new_waiting(cfg: Config) -> Self {
        let mut app = Self::new(cfg);
        app.screen = Screen::Startup;
        app
    }
    pub fn phase(&self) -> PomodoroTask {
        self.task
    }
    pub fn remaining_secs(&self) -> u64 {
        self.remaining
    }
    pub fn progress(&self) -> f32 {
        self.progress_ratio() as f32
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn is_startup(&self) -> bool {
        matches!(self.screen, Screen::Startup)
    }
    pub fn is_summary(&self) -> bool {
        matches!(self.screen, Screen::Summary { .. })
    }
    pub fn transition_message(&self) -> Option<&str> {
        match &self.screen {
            Screen::Transition { message, .. } => Some(message),
            _ => None,
        }
    }

    /// Settings are committed before a session so an active clock never jumps.
    pub fn configure(&mut self, cfg: Config) {
        if self.is_startup() {
            notification::set_muted(cfg.muted);
            self.cfg = cfg;
            self.reset_phase_clock();
        }
    }

    pub fn stats(&self) -> &SessionStats {
        &self.stats
    }

    fn reset_phase_clock(&mut self) {
        self.phase_total = self.task.duration(&self.cfg);
        self.phase_start = Instant::now();
        self.paused_total = Duration::ZERO;
        self.pause_started = None;
        self.paused = false;
        self.remaining = self.phase_total;
    }

    fn elapsed(&self) -> Duration {
        if self.paused {
            if let Some(started) = self.pause_started {
                started
                    .duration_since(self.phase_start)
                    .saturating_sub(self.paused_total)
            } else {
                self.phase_start.elapsed().saturating_sub(self.paused_total)
            }
        } else {
            self.phase_start.elapsed().saturating_sub(self.paused_total)
        }
    }

    pub fn pause(&mut self) {
        if !self.paused && matches!(self.screen, Screen::Running) {
            self.paused = true;
            self.pause_started = Some(Instant::now());
        }
    }

    pub fn resume(&mut self) {
        if self.paused {
            if let Some(started) = self.pause_started {
                self.paused_total += started.elapsed();
            }
            self.paused = false;
            self.pause_started = None;
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.paused {
            self.resume();
        } else {
            self.pause();
        }
    }

    fn finish_transition(&mut self) {
        if let Some(next) = self.pending.take() {
            self.task = next;
            self.reset_phase_clock();
        }
        self.screen = Screen::Running;
    }

    fn begin_transition(&mut self, finished: PomodoroTask, next: PomodoroTask) {
        let message = match finished {
            PomodoroTask::Work => {
                let kind = if next == PomodoroTask::LongBreak {
                    "long break"
                } else {
                    "short break"
                };
                format!(
                    "Focus complete! Take a {} {} {} — {}",
                    fmt_minutes(next.duration(&self.cfg) / 60),
                    kind,
                    next.emoji(),
                    next.tagline(false)
                )
            }
            _ => format!(
                "Break over! {} of focus starting {} — {}",
                fmt_minutes(self.cfg.work_mins),
                PomodoroTask::Work.emoji(),
                PomodoroTask::Work.tagline(false)
            ),
        };
        self.pending = Some(next);
        if self.cfg.auto_start {
            self.finish_transition();
        } else {
            self.screen = Screen::Transition {
                message,
                deadline: Instant::now() + Duration::from_millis(3000),
            };
        }
    }

    fn complete_phase(&mut self) {
        let finished = self.task;
        let (next, _) = change_state(&finished, self.stats.work_completed, self.cfg.cycles.max(1));
        self.stats.record(&finished, &self.cfg);
        notification::play_notification_sound();
        if self.cfg.desktop {
            let (title, body) = match finished {
                PomodoroTask::Work => (
                    "Pomodoro: break time!",
                    match next {
                        PomodoroTask::LongBreak => "Focus session done — enjoy a long break.",
                        _ => "Focus session done — take a short break.",
                    },
                ),
                _ => (
                    "Pomodoro: back to focus!",
                    "Break over — time for the next focus session.",
                ),
            };
            notification::desktop_notify(title, body);
        }
        if matches!(finished, PomodoroTask::LongBreak) {
            self.screen = Screen::Summary { quit_early: false };
            self.done = false; // wait for confirmation
        } else {
            self.begin_transition(finished, next);
        }
    }

    fn skip_phase(&mut self) {
        let finished = self.task;
        let (next, _) = change_state(&finished, self.stats.work_completed, self.cfg.cycles.max(1));
        if matches!(finished, PomodoroTask::LongBreak) {
            self.screen = Screen::Summary { quit_early: false };
        } else {
            // Skips don't count toward stats — straight to transition.
            self.begin_transition_skipped(finished, next);
        }
    }

    fn begin_transition_skipped(&mut self, _finished: PomodoroTask, next: PomodoroTask) {
        self.pending = Some(next);
        if self.cfg.auto_start {
            self.finish_transition();
        } else {
            self.screen = Screen::Transition {
                message: format!(
                    "Skipped — moving to {} {} ...",
                    next.short_name().to_lowercase(),
                    next.emoji()
                ),
                deadline: Instant::now() + Duration::from_millis(1200),
            };
        }
    }

    pub fn handle_action(&mut self, action: crate::key_handler::KeyAction) {
        use crate::key_handler::KeyAction::*;
        match &self.screen {
            Screen::Startup => match action {
                Quit => {
                    self.quit_early = true;
                    self.done = true;
                }
                Confirm | TogglePause | Resume => {
                    self.screen = Screen::Running;
                    self.reset_phase_clock();
                }
                MuteToggle => {
                    notification::toggle_muted();
                }
                _ => {}
            },
            Screen::Running => match action {
                Quit => {
                    self.quit_early = true;
                    self.screen = Screen::Summary { quit_early: true };
                }
                Pause => self.pause(),
                Resume => self.resume(),
                TogglePause | Confirm => self.toggle_pause(),
                Skip => self.skip_phase(),
                Restart => self.reset_phase_clock(),
                MuteToggle => {
                    notification::toggle_muted();
                }
            },
            Screen::Transition { .. } => match action {
                Quit => {
                    self.quit_early = true;
                    self.pending = None;
                    self.screen = Screen::Summary { quit_early: true };
                }
                Skip | Confirm | TogglePause => self.finish_transition(),
                MuteToggle => {
                    notification::toggle_muted();
                }
                _ => {}
            },
            Screen::Summary { .. } => match action {
                Quit | Confirm | TogglePause => {
                    self.done = true;
                }
                _ => {}
            },
        }
    }

    /// Advance the session clock. Called every tick.
    pub fn update(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        match &self.screen {
            Screen::Running => {
                let elapsed = self.elapsed();
                let total = Duration::from_secs(self.phase_total);
                self.remaining = total.saturating_sub(elapsed).as_secs();
                if total.saturating_sub(elapsed).is_zero() {
                    self.complete_phase();
                }
            }
            Screen::Transition { deadline, .. } => {
                if Instant::now() >= *deadline {
                    self.finish_transition();
                }
            }
            _ => {}
        }
    }

    fn progress_ratio(&self) -> f64 {
        if self.phase_total == 0 {
            return 0.0;
        }
        let elapsed = self.phase_total.saturating_sub(self.remaining) as f64;
        (elapsed / self.phase_total as f64).clamp(0.0, 1.0)
    }

    fn round_no(&self) -> u64 {
        match self.task {
            PomodoroTask::Work => self.stats.work_completed + 1,
            _ => self.stats.work_completed.max(1),
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        if area.width < 60 || area.height < 20 {
            let warn = Paragraph::new("terminal too small — make it 60x20+")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            frame.render_widget(warn, area);
            return;
        }

        let accent = self.task.color();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(area);

        self.draw_header(frame, chunks[0], accent);
        match &self.screen {
            Screen::Startup => self.draw_startup(frame, chunks[1], accent),
            Screen::Summary { quit_early } => self.draw_summary(frame, chunks[1], *quit_early),
            _ => self.draw_main(frame, chunks[1], accent),
        }
        self.draw_gauge(frame, chunks[2], accent);

        match &self.screen {
            Screen::Transition { message, .. } => {
                self.draw_transition_bar(frame, chunks[3], message)
            }
            _ => self.draw_footer(frame, chunks[3]),
        }
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect, accent: Color) {
        let sound = if notification::is_muted() {
            "🔇 muted"
        } else {
            "🔊 on"
        };
        let title = format!(
            "🍅 POMODORO  ·  {}  ·  #{}/{}  ·  {}  ·  {}",
            self.task.short_name(),
            self.round_no(),
            self.cfg.cycles,
            fmt_minutes(self.phase_total / 60),
            sound
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .title(Span::styled(
                " focus ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
        let p = Paragraph::new(Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )))
        .block(block)
        .alignment(Alignment::Center);
        frame.render_widget(p, area);
    }

    fn draw_main(&self, frame: &mut Frame, area: Rect, accent: Color) {
        if area.width >= 100 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
                .split(area);
            self.draw_clock(frame, cols[0], accent);
            self.draw_info(frame, cols[1], accent);
        } else {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(9), Constraint::Length(9)])
                .split(area);
            self.draw_clock(frame, rows[0], accent);
            self.draw_info(frame, rows[1], accent);
        }
    }

    fn draw_clock(&self, frame: &mut Frame, area: Rect, accent: Color) {
        let lines = big_time_lines(&self.task, self.remaining);
        let text: Vec<Line> = lines
            .into_iter()
            .map(|l| {
                Line::from(Span::styled(
                    l,
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))
            })
            .collect();
        let label = if self.paused {
            " Paused "
        } else {
            self.task.tagline(false)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .title(Span::styled(label, Style::default().fg(Color::White)));
        let p = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(p, area);
    }

    fn draw_info(&self, frame: &mut Frame, area: Rect, accent: Color) {
        let phase = format!("{}  {}", self.task.emoji(), self.task.description());
        let status = if self.paused {
            "⏸ PAUSED — press Space to resume".to_string()
        } else {
            format!("● LIVE — {}", self.task.short_name())
        };
        let cycle = format!(
            "{}  session {}/{}",
            cycle_tracker(self.stats.work_completed, self.cfg.cycles, &self.task),
            self.round_no(),
            self.cfg.cycles
        );
        let next = next_hint(&self.task, &self.cfg, self.stats.work_completed);
        let stats = format!(
            "🍅 {} pomodoros  ·  {} focus",
            self.stats.work_completed,
            fmt_total_minutes(self.stats.focus_secs)
        );
        let stats2 = format!(
            "☕ {} break  ·  {} elapsed",
            fmt_total_minutes(self.stats.break_secs),
            fmt_total_minutes(self.stats.session_elapsed_secs())
        );
        let detail = format!("⏳ {} remaining", fmt_mmss(self.remaining));
        let lines = vec![
            Line::from(Span::styled(
                status,
                Style::default()
                    .fg(if self.paused {
                        Color::Yellow
                    } else {
                        Color::Green
                    })
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::raw(phase)),
            Line::from(Span::styled(cycle, Style::default().fg(accent))),
            Line::from(Span::raw(next)),
            Line::from(Span::raw(stats)),
            Line::from(Span::raw(stats2)),
            Line::from(Span::styled(
                detail,
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .title(" session ");
        let p = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(p, area);
    }

    fn draw_gauge(&self, frame: &mut Frame, area: Rect, accent: Color) {
        let label = format!(
            "{:>3}% — {} remaining",
            (self.progress_ratio() * 100.0).round() as u64,
            fmt_mmss(self.remaining)
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent));
        let gauge = Gauge::default()
            .block(block)
            .gauge_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .ratio(self.progress_ratio())
            .label(Span::styled(label, Style::default().fg(Color::White)));
        frame.render_widget(gauge, area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" pause/resume · "),
            Span::styled("S", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" skip · "),
            Span::styled("X", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" restart · "),
            Span::styled("M", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" mute · "),
            Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" quit"),
        ]);
        let p = Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(p, area);
    }

    fn draw_transition_bar(&self, frame: &mut Frame, area: Rect, message: &str) {
        let p = Paragraph::new(Line::from(Span::styled(
            format!("{message}   (S skip · Q quit)"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
        frame.render_widget(p, area);
    }

    fn draw_startup(&self, frame: &mut Frame, area: Rect, accent: Color) {
        let popup = centered_rect(76, 11, area);
        let mut plan = vec![Line::from(Span::styled(
            "POMODORO — READY TO FOCUS?",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))];
        plan.extend(vec![
            Line::from(Span::raw(format!(
                "focus {}  ·  short break {}  ·  long break {}  ·  {} sessions",
                fmt_minutes(self.cfg.work_mins),
                fmt_minutes(self.cfg.short_mins),
                fmt_minutes(self.cfg.long_mins),
                self.cfg.cycles
            ))),
            Line::from(Span::raw(format!(
                "up to {} of focus  ·  sound: {}",
                fmt_total_minutes(self.cfg.total_focus_secs()),
                if notification::is_muted() {
                    "muted"
                } else {
                    "on"
                }
            ))),
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "press SPACE / ENTER to start  ·  Q to quit",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent));
        let p = Paragraph::new(plan)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(p, popup);
    }

    fn draw_summary(&self, frame: &mut Frame, area: Rect, quit_early: bool) {
        let popup = centered_rect(76, 17, area);
        let headline = if self.stats.work_completed == 0 {
            "No pomodoros finished — showing up still counts.".to_string()
        } else if quit_early {
            format!(
                "Session ended early — {} pomodoro{} completed.",
                self.stats.work_completed,
                if self.stats.work_completed == 1 {
                    ""
                } else {
                    "s"
                }
            )
        } else {
            format!(
                "Session complete — {} pomodoro{} finished!",
                self.stats.work_completed,
                if self.stats.work_completed == 1 {
                    ""
                } else {
                    "s"
                }
            )
        };
        let detail = format!(
            "{} focus  ·  {} break  ·  {} elapsed",
            fmt_total_minutes(self.stats.focus_secs),
            fmt_total_minutes(self.stats.break_secs),
            fmt_total_minutes(self.stats.session_elapsed_secs())
        );
        let mut lines = vec![Line::from(Span::styled(
            headline,
            Style::default().add_modifier(Modifier::BOLD),
        ))];
        lines.extend(vec![
            Line::from(Span::raw(detail)),
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "press Q / ENTER to exit",
                Style::default().fg(Color::Gray),
            )),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(200, 130, 60)))
            .title(" session complete ");
        let p = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(p, popup);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn harness() -> (Terminal<TestBackend>, App) {
        let backend = TestBackend::new(110, 40);
        let terminal = Terminal::new(backend).unwrap();
        let mut cfg = Config::default();
        cfg.auto_start = true;
        let app = App::new(cfg);
        (terminal, app)
    }

    #[test]
    fn pause_resume_restart_and_skip_preserve_clock_semantics() {
        let (_, mut app) = harness();
        app.phase_start = Instant::now() - Duration::from_secs(35);
        app.update();
        assert!(app.remaining <= app.phase_total - 35);
        app.pause();
        let frozen = app.remaining;
        app.update();
        assert_eq!(app.remaining, frozen);
        app.resume();
        assert!(!app.paused);
        app.handle_action(crate::key_handler::KeyAction::Restart);
        assert_eq!(app.remaining, app.phase_total);
        app.handle_action(crate::key_handler::KeyAction::Skip);
        assert_eq!(app.task, PomodoroTask::ShortBreak);
        assert_eq!(app.stats.work_completed, 0);
    }

    #[test]
    fn settings_cannot_change_an_active_session() {
        let (_, mut app) = harness();
        let original = app.remaining;
        let mut cfg = app.cfg.clone();
        cfg.work_mins = 50;
        app.configure(cfg);
        assert_eq!(app.remaining, original);
        assert_eq!(app.cfg.work_mins, 25);
    }

    #[test]
    fn draws_running_timer() {
        let (mut terminal, app) = harness();
        terminal.draw(|f| app.draw(f)).unwrap();
        let plain: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(plain.contains("POMODORO"), "header missing");
        assert!(plain.contains("session"), "status panel missing");
        assert!(plain.contains("remaining"), "gauge missing");
    }

    #[test]
    fn draws_startup_summary_and_transition() {
        let backend = TestBackend::new(110, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal.draw(|f| app.draw(f)).unwrap();

        let mut auto_cfg = Config::default();
        auto_cfg.auto_start = true;
        let mut app2 = App::new(auto_cfg);
        app2.screen = Screen::Transition {
            message: "test treats".to_string(),
            deadline: Instant::now() + Duration::from_secs(5),
        };
        terminal.draw(|f| app2.draw(f)).unwrap();

        app2.screen = Screen::Summary { quit_early: false };
        terminal.draw(|f| app2.draw(f)).unwrap();
    }
}
