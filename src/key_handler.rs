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
    if event::poll(Duration::from_millis(100)).expect("Poll failed") {
        match event::read().expect("Read failed") {
            Event::Key(key_event) => {
                let action = match key_event.code {
                    Esc => Some(KeyAction::Quit),
                    Char('p') => Some(KeyAction::Pause),
                    Char('r') => Some(KeyAction::Resume),
                    _ => None,
                };

                action
            }
            _ => None,
        }
    } else {
        None
    }
}
