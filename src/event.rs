use crate::key_handler::*;
use std::io::Write;

use crossterm::{
    cursor::{Hide, MoveToColumn, MoveUp, Show},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::stdout;
use std::time::{Duration, Instant};

use cfonts::{render, Colors, Fonts, Options};
use unicode_width::UnicodeWidthStr;

#[derive(PartialEq)]
pub enum PomodoroTask {
    Work,
    ShortBreak,
    LongBreak,
}

impl PomodoroTask {
    pub fn duration(&self) -> u16 {
        match self {
            PomodoroTask::Work => 25 * 60,
            PomodoroTask::ShortBreak => 5 * 60,
            PomodoroTask::LongBreak => 15 * 60,
        }
    }

    fn emoji(&self) -> String {
        match self {
            PomodoroTask::Work => "✍️".to_string(),
            PomodoroTask::ShortBreak => "🧘".to_string(),
            PomodoroTask::LongBreak => "💆".to_string(),
        }
    }

    fn description(&self) -> String {
        match self {
            PomodoroTask::Work => "It's time to work".to_string(),
            PomodoroTask::ShortBreak => "It's time for a short break".to_string(),
            PomodoroTask::LongBreak => "It's time for a long break".to_string(),
        }
    }

    fn color(&self) -> Color {
        match self {
            PomodoroTask::Work => Color::Red,
            PomodoroTask::ShortBreak => Color::Green,
            PomodoroTask::LongBreak => Color::Blue,
        }
    }

    fn gradient(&self) -> Vec<Colors> {
        match self {
            PomodoroTask::Work => vec![Colors::Red, Colors::Yellow],
            PomodoroTask::ShortBreak => vec![Colors::Green, Colors::Red],
            PomodoroTask::LongBreak => vec![Colors::Blue, Colors::Red],
        }
    }
}

pub fn countdown(stdout: &mut impl Write, state: &PomodoroTask, is_paused: &mut bool) -> bool {
    let start_time = Instant::now();
    let total_duration = Duration::from_secs(state.duration() as u64);
    let mut last_displayed_seconds: Option<u64> = None;
    let mut pause_start: Option<Instant> = None;
    let mut total_paused_duration = Duration::ZERO;

    loop {
        // Calculate elapsed time accounting for pauses
        let elapsed = if *is_paused {
            // While paused, freeze the elapsed time
            if let Some(pause_time) = pause_start {
                pause_time.duration_since(start_time) - total_paused_duration
            } else {
                start_time.elapsed() - total_paused_duration
            }
        } else {
            start_time.elapsed() - total_paused_duration
        };

        // Calculate remaining time with drift correction
        let remaining = if elapsed < total_duration {
            total_duration - elapsed
        } else {
            Duration::ZERO
        };

        let remaining_seconds = remaining.as_secs();

        // Check if timer is complete
        if remaining_seconds == 0 && remaining.as_millis() == 0 {
            clear_display(stdout);
            return false;
        }

        // Only update display when seconds actually change (avoid unnecessary redraws)
        if last_displayed_seconds != Some(remaining_seconds) {
            display_ascii_clock(stdout, state, remaining_seconds, *is_paused);
            last_displayed_seconds = Some(remaining_seconds);
        }

        // Non-blocking event poll with 50ms timeout for responsive input
        if let Some(key_action) = read_keystroke() {
            match key_action {
                KeyAction::Pause => {
                    if !*is_paused {
                        *is_paused = true;
                        pause_start = Some(Instant::now());
                    }
                }
                KeyAction::Resume => {
                    if *is_paused {
                        if let Some(pause_time) = pause_start {
                            total_paused_duration += pause_time.elapsed();
                        }
                        *is_paused = false;
                        pause_start = None;
                    }
                }
                KeyAction::Quit => {
                    clear_display(stdout);
                    return true;
                }
            }
        }
    }
}

/// Display large ASCII art clock with colorful styling, borders, and progress bar
fn display_ascii_clock(
    stdout: &mut impl Write,
    state: &PomodoroTask,
    remaining_seconds: u64,
    is_paused: bool,
) {
    let minutes = remaining_seconds / 60;
    let seconds = remaining_seconds % 60;
    let time_str = format!("{:02}:{:02}", minutes, seconds);

    // Generate ASCII art for the time using cfonts with gradient
    let ascii_art = {
        let mut options = Options::default();
        options.font = Fonts::FontBlock;
        options.colors = state.gradient();
        options.text = time_str.clone();

        let rendered = render(options);
        rendered.text
    };

    let lines: Vec<&str> = ascii_art.lines().collect();

    // Fixed width for consistent display (inner content width, not including borders)
    let inner_width = 58;

    // Calculate progress
    let total_seconds = state.duration() as u64;
    let elapsed_seconds = total_seconds - remaining_seconds;
    let progress_percent = (elapsed_seconds as f64 / total_seconds as f64 * 100.0) as u64;
    let progress_bar_width: usize = 40;
    let filled_width = (progress_bar_width as f64 * (progress_percent as f64 / 100.0)) as usize;

    // Total number of lines in the boxed clock (borders, header, ASCII art, progress bar)
    let total_display_lines = lines.len() + 9;

    // Move cursor up to the start of the reserved display area and to column 0
    let _ = execute!(stdout, MoveUp(total_display_lines as u16), MoveToColumn(0));

    // Top border with title
    let top_border = format!("╔{}╗", "═".repeat(inner_width));
    let title_text = "🍅 POMODORO TIMER";
    let title_width = UnicodeWidthStr::width(title_text);
    let title_padding = inner_width.saturating_sub(title_width);
    let title = format!("║{}{}║", title_text, " ".repeat(title_padding));
    let separator = format!("╠{}╣", "═".repeat(inner_width));

    let _ = execute!(
        stdout,
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(state.color()),
        Print(&top_border),
        Print("\n"),
        terminal::Clear(ClearType::CurrentLine),
        Print(&title),
        Print("\n"),
        terminal::Clear(ClearType::CurrentLine),
        Print(&separator),
        Print("\n"),
    );

    // Display header with task info
    let status_icon = if is_paused {
        "⏸️ PAUSED"
    } else {
        &state.emoji()
    };
    let header_text = format!("{} {}", state.description(), status_icon);
    let header_width = header_text.as_str().width();
    let header_padding = inner_width.saturating_sub(header_width);
    let header = format!("║{}{}║", header_text, " ".repeat(header_padding));

    let _ = execute!(
        stdout,
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(state.color()),
        Print(&header),
        Print("\n"),
        terminal::Clear(ClearType::CurrentLine),
        Print(&separator),
        Print("\n"),
    );

    // Display ASCII art clock line by line with borders
    for line in &lines {
        let line_len = visible_width(line);
        let content_padding = inner_width.saturating_sub(line_len);
        let left_pad = content_padding / 2;
        let right_pad = content_padding - left_pad;
        let _ = execute!(
            stdout,
            terminal::Clear(ClearType::CurrentLine),
            SetForegroundColor(state.color()),
            Print("║"),
            Print(" ".repeat(left_pad)),
            ResetColor,
            Print(line),
            SetForegroundColor(state.color()),
            Print(" ".repeat(right_pad)),
            Print("║"),
            Print("\n"),
        );
    }

    // Progress bar
    let progress_text = format!(
        "Progress: [{}{}] {}%",
        "█".repeat(filled_width),
        "░".repeat(progress_bar_width.saturating_sub(filled_width)),
        progress_percent
    );
    let progress_width = progress_text.as_str().width();
    let progress_padding = inner_width.saturating_sub(progress_width);
    let progress_bar = format!("║{}{}║", progress_text, " ".repeat(progress_padding));

    let _ = execute!(
        stdout,
        terminal::Clear(ClearType::CurrentLine),
        Print(&separator),
        Print("\n"),
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(state.color()),
        Print(&progress_bar),
        Print("\n"),
    );

    // Bottom border
    let bottom_border = format!("╚{}╝", "═".repeat(inner_width));
    let _ = execute!(
        stdout,
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(state.color()),
        Print(&bottom_border),
        Print("\n"),
        terminal::Clear(ClearType::CurrentLine),
        ResetColor,
        Print("\n"),
    );

    let _ = stdout.flush();
}

/// Compute the visible width of a string, ignoring ANSI color escape sequences
fn visible_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;

    let mut cleaned = String::with_capacity(s.len());
    let mut in_escape = false;

    for ch in s.chars() {
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }

        if ch == '\u{1b}' {
            // Start of ANSI escape sequence
            in_escape = true;
            continue;
        }

        cleaned.push(ch);
    }

    cleaned.width()
}

/// Clear the display area and restore cursor
fn clear_display(stdout: &mut impl Write) {
    let _ = execute!(stdout, terminal::Clear(ClearType::CurrentLine), Show,);
    let _ = stdout.flush();
}

pub fn print_empty_line() {
    let _ = execute!(stdout(), Print("\n"));
}

/// Initialize display area for ASCII clock (reserves vertical space)
pub fn init_display(stdout: &mut impl Write) {
    let _ = execute!(stdout, Hide);
    for _ in 0..20 {
        let _ = execute!(stdout, Print("\n"));
    }
    let _ = stdout.flush();
}

/// Cleanup display on exit (restore cursor visibility)
pub fn cleanup_display(stdout: &mut impl Write) {
    let _ = execute!(stdout, Show, Print("\n"));
    let _ = stdout.flush();
}

pub fn change_state(state: &PomodoroTask, rounds: u16) -> (PomodoroTask, u16) {
    let mut rounds = rounds;
    let new_state = match state {
        PomodoroTask::Work => {
            rounds += 1;
            if rounds % 4 == 0 {
                PomodoroTask::LongBreak
            } else {
                PomodoroTask::ShortBreak
            }
        }
        PomodoroTask::ShortBreak | PomodoroTask::LongBreak => PomodoroTask::Work,
    };
    (new_state, rounds)
}
