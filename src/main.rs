mod event;
mod key_handler;
mod notification;

use crate::event::*;
use crate::key_handler::*;
use crossterm::style::Stylize;
use notification::*;
use std::io::stdout;

fn main() {
    let mut stdout = stdout();

    let mut rounds = 0;
    let mut task = PomodoroTask::Work;

    println!("{}", "🍅 Pomodoro App".bold());
    println!(
        "Press {} to quit, {} to pause, and {} to resume.",
        "ESC".italic(),
        'p'.italic(),
        'r'.italic()
    );
    let mut is_paused = false;

    loop {
        // Check for user input
        if let Some(key_action) = read_keystroke() {
            match key_action {
                KeyAction::Pause => {
                    is_paused = !is_paused;
                }

                KeyAction::Quit => {
                    break;
                }
                _ => {}
            }
        }

        // If the application is paused, enter a "wait" state
        while is_paused {
            if let Some(key_action) = read_keystroke() {
                if let KeyAction::Pause = key_action {
                    is_paused = false;
                }
            }
        }

        // Normal operation of the application
        if countdown(&mut stdout, &task, &mut is_paused) {
            print_empty_line();
            break;
        };

        let (new_task, new_rounds) = change_state(&task, rounds);
        task = new_task;
        rounds = new_rounds;
        play_notification_sound();
    }

    match rounds {
        rounds if rounds > 1 => {
            println!(
                "\n🎉 Congratulations! \nYou've successfully completed {} pomodoros, {:.0} minutes in total.",
                rounds,
                ((PomodoroTask::Work.duration() * rounds) / 60) as f32
            );
        }
        rounds if rounds < 1 => {
            println!(
                "\nYou started a pomodoro, but it seems you haven't completed one yet. Keep going, you're doing great!"
            );
        }
        _ => {
            println!(
                "\n🎉 Congratulations! \nYou've successfully completed {} pomodoro.",
                rounds,
            );
        }
    }
}
