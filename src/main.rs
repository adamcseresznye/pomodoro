mod event;
mod key_handler;
mod notification;

use crate::event::*;
use crate::key_handler::*;
use crossterm::style::Stylize;
use notification::*;

fn main() {
    let mut rounds = 0;
    let mut state = PomodoroState::Work;

    println!("{}", "🍅 Pomodoro App".bold());
    println!("Press {} to quit.", "ESC".italic());
    loop {
        if let Some(action) = countdown(&state) {
            match action {
                KeyAction::Quit => {
                    print_empty_line();
                    break;
                }
            }
        };
        let (new_state, new_rounds) = change_state(&state, rounds);
        state = new_state;
        rounds = new_rounds;
        play_notification_sound();
    }
    println!(
        "🎉 Congratulations! \nYou completed {} pomodoros, {} min in total.",
        rounds,
        PomodoroState::Work.duration() * rounds
    );
}
