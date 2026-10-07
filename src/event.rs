use crate::config::Config;
use cfonts::{render, Colors, Fonts, Options};
use ratatui::style::Color;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PomodoroTask {
    Work,
    ShortBreak,
    LongBreak,
}

impl PomodoroTask {
    pub fn duration(&self, cfg: &Config) -> u64 {
        match self {
            PomodoroTask::Work => cfg.work_mins * 60,
            PomodoroTask::ShortBreak => cfg.short_mins * 60,
            PomodoroTask::LongBreak => cfg.long_mins * 60,
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            PomodoroTask::Work => "FOCUS",
            PomodoroTask::ShortBreak => "SHORT BREAK",
            PomodoroTask::LongBreak => "LONG BREAK",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            PomodoroTask::Work => "🍅",
            PomodoroTask::ShortBreak => "☕",
            PomodoroTask::LongBreak => "🌙",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PomodoroTask::Work => "Focus time — stay in the zone",
            PomodoroTask::ShortBreak => "Short break — stretch and breathe",
            PomodoroTask::LongBreak => "Long break — rest and recharge",
        }
    }

    /// Natural, realistic accent used across the Ratatui theme.
    /// Amber focus, sage short break, slate long break.
    pub fn color(&self) -> Color {
        match self {
            PomodoroTask::Work => Color::Rgb(215, 140, 60),
            PomodoroTask::ShortBreak => Color::Rgb(130, 165, 120),
            PomodoroTask::LongBreak => Color::Rgb(120, 145, 175),
        }
    }

    pub fn gradient(&self) -> Vec<Colors> {
        match self {
            PomodoroTask::Work => vec![Colors::Yellow, Colors::Red],
            PomodoroTask::ShortBreak => vec![Colors::Green, Colors::Yellow],
            PomodoroTask::LongBreak => vec![Colors::Blue, Colors::Gray],
        }
    }

    pub fn tagline(&self, is_paused: bool) -> &'static str {
        if is_paused {
            "Paused — press Space to resume"
        } else {
            match self {
                PomodoroTask::Work => "Stay focused — you've got this",
                PomodoroTask::ShortBreak => "Break time — you earned it",
                PomodoroTask::LongBreak => "Long break — switch off for a while",
            }
        }
    }
}

/// Session-wide stats shown in the HUD and final summary.
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub work_completed: u64,
    pub focus_secs: u64,
    pub break_secs: u64,
    pub session_start: std::time::Instant,
}

impl SessionStats {
    pub fn new() -> Self {
        Self {
            work_completed: 0,
            focus_secs: 0,
            break_secs: 0,
            session_start: std::time::Instant::now(),
        }
    }

    pub fn record(&mut self, task: &PomodoroTask, cfg: &Config) {
        match task {
            PomodoroTask::Work => {
                self.work_completed += 1;
                self.focus_secs += task.duration(cfg);
            }
            _ => {
                self.break_secs += task.duration(cfg);
            }
        }
    }

    pub fn session_elapsed_secs(&self) -> u64 {
        self.session_start.elapsed().as_secs()
    }
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}

pub fn change_state(state: &PomodoroTask, rounds: u64, cycles: u64) -> (PomodoroTask, u64) {
    let mut rounds = rounds;
    let new_state = match state {
        PomodoroTask::Work => {
            rounds += 1;
            if rounds % cycles == 0 {
                PomodoroTask::LongBreak
            } else {
                PomodoroTask::ShortBreak
            }
        }
        PomodoroTask::ShortBreak | PomodoroTask::LongBreak => PomodoroTask::Work,
    };
    (new_state, rounds)
}

pub fn fmt_mmss(total_secs: u64) -> String {
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

pub fn fmt_minutes(mins: u64) -> String {
    format!("{mins}m")
}

pub fn fmt_total_minutes(secs: u64) -> String {
    let mins = secs / 60;
    if mins >= 60 {
        format!("{}h {:02}m", mins / 60, mins % 60)
    } else {
        format!("{mins}m")
    }
}

/// Cycle tracker: completed sessions vs empty dots.
pub fn cycle_tracker(work_completed: u64, cycles: u64, current: &PomodoroTask) -> String {
    let filled = match current {
        PomodoroTask::Work => work_completed % cycles,
        _ => {
            let r = work_completed % cycles;
            if r == 0 {
                cycles
            } else {
                r
            }
        }
    };
    (0..cycles)
        .map(|i| if i < filled { "●" } else { "○" })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn next_hint(task: &PomodoroTask, cfg: &Config, work_completed: u64) -> String {
    match task {
        PomodoroTask::Work => {
            if (work_completed + 1) % cfg.cycles == 0 {
                format!("Next: long break ({})", fmt_minutes(cfg.long_mins))
            } else {
                format!("Next: short break ({})", fmt_minutes(cfg.short_mins))
            }
        }
        _ => format!("Next: focus ({})", fmt_minutes(cfg.work_mins)),
    }
}

/// Strip ANSI escape sequences (cfonts colors) so Ratatui can style text itself.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        if ch == '\u{1b}' {
            in_escape = true;
            continue;
        }
        out.push(ch);
    }
    out
}

/// Big block-digit lines for the clock, ANSI-stripped for Ratatui styling.
pub fn big_time_lines(task: &PomodoroTask, remaining_seconds: u64) -> Vec<String> {
    let time_str = fmt_mmss(remaining_seconds);
    let mut options = Options::default();
    options.font = Fonts::FontBlock;
    options.colors = task.gradient();
    options.text = time_str;
    render(options).text.lines().map(strip_ansi).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn work_short_long_cycle() {
        let c = cfg();
        let (s1, r1) = change_state(&PomodoroTask::Work, 0, c.cycles);
        assert_eq!((s1, r1), (PomodoroTask::ShortBreak, 1));
        let (s2, r2) = change_state(&s1, r1, c.cycles);
        assert_eq!((s2, r2), (PomodoroTask::Work, 1));
        let (s, r) = change_state(&PomodoroTask::Work, 3, 4);
        assert_eq!((s, r), (PomodoroTask::LongBreak, 4));
    }

    #[test]
    fn durations_follow_config() {
        let mut c = cfg();
        c.work_mins = 50;
        assert_eq!(PomodoroTask::Work.duration(&c), 3000);
    }

    #[test]
    fn mmss_formats() {
        assert_eq!(fmt_mmss(0), "00:00");
        assert_eq!(fmt_mmss(65), "01:05");
        assert_eq!(fmt_mmss(1500), "25:00");
    }

    #[test]
    fn big_clock_renders_lines_without_ansi() {
        let lines = big_time_lines(&PomodoroTask::Work, 1500);
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|l| !l.contains('\u{1b}')));
    }
}
