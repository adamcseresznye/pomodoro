use crate::key_handler::*;
use std::io::Write;

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
}

pub fn countdown(stdout: &mut impl Write, state: &PomodoroTask, is_paused: &mut bool) -> bool {
    for time in (1..=state.duration()).rev() {
        let countdown_message = format!(
            "{} {}  Time remaining: {}.",
            &state.description(),
            &state.emoji(),
            convert_to_min(time)
        );
        display(stdout, countdown_message, &state.color());
        spin_sleep::sleep(Duration::new(0, 900_000_000)); //sleep for 0.9 sec

        // Check for user input
        if let Some(key_action) = read_keystroke() {
            //read_keystroke polling for 0.1 sec
            match key_action {
                KeyAction::Pause => {
                    *is_paused = true;
                }
                KeyAction::Resume => {
                    *is_paused = false;
                }
                KeyAction::Quit => {
                    return true;
                }
            }
        }

        // If the countdown is paused, enter a "wait" state
        while *is_paused {
            if let Some(key_action) = read_keystroke() {
                if let KeyAction::Resume = key_action {
                    *is_paused = false;
                }
            }
        }
    }
    return false;
}

fn display(stdout: &mut impl Write, message: String, color: &Color) {
    execute!(
        stdout,
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
