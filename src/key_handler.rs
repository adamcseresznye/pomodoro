use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum KeyAction {
    Quit,
    Pause,
    Resume,
    TogglePause,
    Skip,
    Restart,
    MuteToggle,
    Confirm,
}

/// Map a key event to an action. Only key-presses count (ignores release/repeat).
pub fn map_key(key: KeyEvent) -> Option<KeyAction> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match key.code {
        KeyCode::Esc => Some(KeyAction::Quit),
        KeyCode::Char('q') | KeyCode::Char('Q') => Some(KeyAction::Quit),
        KeyCode::Char('p') | KeyCode::Char('P') => Some(KeyAction::Pause),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(KeyAction::Resume),
        KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('n') | KeyCode::Char('N') => {
            Some(KeyAction::Skip)
        }
        KeyCode::Char('m') | KeyCode::Char('M') => Some(KeyAction::MuteToggle),
        KeyCode::Char('x') | KeyCode::Char('X') => Some(KeyAction::Restart),
        KeyCode::Char(' ') => Some(KeyAction::TogglePause),
        KeyCode::Enter => Some(KeyAction::Confirm),
        _ => None,
    }
}
