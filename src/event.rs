use crate::key_handler::*;

use crossterm::{
    cursor::{Hide, RestorePosition, SavePosition},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::stdout;

use spin_sleep;
use std::time::Duration;

#[derive(PartialEq)]
pub enum PomodoroState {
    Work,
    ShortBreak,
    LongBreak,
}

impl PomodoroState {
    pub fn duration(&self) -> u16 {
        match self {
            PomodoroState::Work => 25 * 60,
            PomodoroState::ShortBreak => 5 * 60,
            PomodoroState::LongBreak => 15 * 60,
        }
    }

    fn emoji(&self) -> String {
        match self {
            PomodoroState::Work => "✍️".to_string(),
            PomodoroState::ShortBreak => "🧘".to_string(),
            PomodoroState::LongBreak => "💆".to_string(),
        }
    }

    fn description(&self) -> String {
        match self {
            PomodoroState::Work => "It's time to work".to_string(),
            PomodoroState::ShortBreak => "It's time for a short break".to_string(),
            PomodoroState::LongBreak => "It's time for a long break".to_string(),
        }
    }

    fn color(&self) -> Color {
        match self {
            PomodoroState::Work => Color::Red,
            PomodoroState::ShortBreak => Color::Green,
            PomodoroState::LongBreak => Color::Blue,
        }
    }
}

pub fn countdown(state: &PomodoroState) -> Option<KeyAction> {
    let mut action = None;
    for time in (1..=state.duration()).rev() {
        let countdown_message = format!(
            "{} {}  Time remaining: {}.",
            &state.description(),
            &state.emoji(),
            convert_to_min(time)
        );
        display(countdown_message, &state.color());
        if let Some(key_action) = read_keystroke() {
            action = Some(key_action);
            break;
        }
        spin_sleep::sleep(Duration::new(1, 0));
    }
    action
}

fn display(message: String, color: &Color) {
    execute!(
        stdout(),
        Hide,
        terminal::Clear(ClearType::CurrentLine),
        SavePosition,
        SetForegroundColor(*color),
        Print(&message),
        RestorePosition,
    )
    .expect("Failed to execute");
}

fn convert_to_min(duration: u16) -> String {
    let min = duration / 60;
    let sec = duration % 60;
    format!("{:2} min {:2} sec", min, sec)
}

pub fn print_empty_line() {
    execute!(stdout(), Print("\n"),).expect("Failed to execute");
}

pub fn change_state(state: &PomodoroState, rounds: u16) -> (PomodoroState, u16) {
    let mut rounds = rounds;
    let new_state = match state {
        PomodoroState::Work => {
            rounds += 1;
            if rounds % 4 == 0 {
                PomodoroState::LongBreak
            } else {
                PomodoroState::ShortBreak
            }
        }
        PomodoroState::ShortBreak | PomodoroState::LongBreak => PomodoroState::Work,
    };
    (new_state, rounds)
}
