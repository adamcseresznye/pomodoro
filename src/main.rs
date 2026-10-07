#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod event;
mod gui;
mod key_handler;
mod notification;
mod soundscape;

use crate::app::{tick_duration, App};
use crate::config::{parse_args, ArgResult};
use crossterm::{
    event::{poll, read, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal_mode = std::env::args().any(|a| a == "--terminal");
    let raw_args: Vec<String> = std::env::args().filter(|a| a != "--terminal").collect();
    let cfg = match parse_args(raw_args) {
        ArgResult::Run(c) => c,
        ArgResult::Help => {
            attach_console();
            print!("{}", crate::config::help_text());
            return Ok(());
        }
        ArgResult::Error(e) => {
            attach_console();
            eprintln!("Error: {e}\n");
            eprint!("{}", crate::config::help_text());
            std::process::exit(2);
        }
    };

    notification::set_muted(cfg.muted);

    if !terminal_mode {
        return gui::run(cfg).map_err(|e| e.to_string().into());
    }
    run_terminal(cfg)
}

fn run_terminal(cfg: crate::config::Config) -> Result<(), Box<dyn std::error::Error>> {
    attach_console();
    enable_raw_mode().expect("raw mode");
    let mut out = stdout();
    execute!(out, EnterAlternateScreen).expect("alt screen");
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend).expect("terminal");

    let mut app = App::new(cfg);

    loop {
        terminal.draw(|f| app.draw(f)).expect("draw");

        if app.done {
            break;
        }

        if poll(tick_duration()).expect("poll") {
            match read().expect("read") {
                Event::Key(key) => {
                    if let Some(action) = crate::key_handler::map_key(key) {
                        app.handle_action(action);
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        // Update the clock between input events.
        app.update();

        // Small sleep guard so a tight poll loop can't spin hot.
        std::thread::sleep(Duration::from_millis(10));
    }

    disable_raw_mode().expect("raw off");
    execute!(terminal.backend_mut(), LeaveAlternateScreen).expect("leave");
    terminal.show_cursor().expect("cursor");

    let stats = app.stats().clone();
    if stats.work_completed == 0 && app.quit_early {
        println!("No problem — come back when you're ready to focus.");
    } else {
        println!(
            "Session over: {} pomodoros · {} focus · {} break",
            stats.work_completed,
            crate::event::fmt_total_minutes(stats.focus_secs),
            crate::event::fmt_total_minutes(stats.break_secs)
        );
    }
    Ok(())
}

// The desktop release opens without a console. CLI modes attach to the
// invoking terminal, or create a console when launched without one.
fn attach_console() {
    #[cfg(all(windows, not(debug_assertions)))]
    unsafe {
        use windows_sys::Win32::System::Console::{
            AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS,
        };
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            AllocConsole();
        }
    }
}
