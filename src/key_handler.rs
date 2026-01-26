use crossterm::event::KeyCode::*;
use crossterm::event::{self, Event};
use std::time::Duration;

#[derive(PartialEq, Copy, Clone)]
pub enum KeyAction {
    Quit,
    Pause,
    Resume,
}

pub fn read_keystroke() -> Option<KeyAction> {
    match event::poll(Duration::from_millis(50)) {
        Ok(true) => {
            match event::read() {
                Ok(Event::Key(key_event)) => match key_event.code {
                    Esc => Some(KeyAction::Quit),
                    Char('p') => Some(KeyAction::Pause),
                    Char('r') => Some(KeyAction::Resume),
                    _ => None,
                },
                Ok(_) => None,  // Non-key event
                Err(_) => None, // Read error, ignore gracefully
            }
        }
        Ok(false) => None, // No event within timeout
        Err(_) => None,    // Poll error, ignore gracefully
    }
}
