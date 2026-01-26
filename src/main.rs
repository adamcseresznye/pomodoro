mod event;
mod key_handler;
mod notification;

use crate::event::*;
use crossterm::style::Stylize;
use notification::*;
use std::io::stdout;

fn main() {
    let mut stdout = stdout();

    let mut rounds = 0;
    let mut task = PomodoroTask::Work;

    // Brief instruction message above the clock box
    println!(
        "Controls: {} quit | {} pause | {} resume\n",
        "ESC".italic(),
        "p".italic(),
        "r".italic()
    );

    // Initialize display area for ASCII clock
    init_display(&mut stdout);

    let mut is_paused = false;

    loop {
        // Normal operation of the application
        if countdown(&mut stdout, &task, &mut is_paused) {
            cleanup_display(&mut stdout);
            print_empty_line();
            break;
        };

        let (new_task, new_rounds) = change_state(&task, rounds);
        task = new_task;
        rounds = new_rounds;
        play_notification_sound();
    }

    // Ensure cursor is restored on normal exit
    cleanup_display(&mut stdout);

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
