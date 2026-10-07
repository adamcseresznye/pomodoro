use once_cell::sync::Lazy;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

static MUTED: AtomicBool = AtomicBool::new(false);

pub fn set_muted(muted: bool) {
    MUTED.store(muted, Ordering::SeqCst);
}

pub fn is_muted() -> bool {
    MUTED.load(Ordering::SeqCst)
}

pub fn toggle_muted() -> bool {
    let next = !is_muted();
    set_muted(next);
    next
}

// Single background audio thread that owns the OutputStream and handles play requests
static AUDIO_TX: Lazy<mpsc::Sender<()>> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        match OutputStream::try_default() {
            Ok((stream, handle)) => {
                let _stream = stream; // keep stream alive in this thread
                while let Ok(_) = rx.recv() {
                    let sound_data = include_bytes!("bell.mp3");
                    let cursor = Cursor::new(sound_data);
                    if let Ok(decoder) = Decoder::new(cursor) {
                        if let Ok(sink) = Sink::try_new(&handle) {
                            sink.append(decoder);
                            sink.detach();
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "⚠️  Audio device not available: {}. Sounds will be muted.",
                    e
                );
                // Drain requests to avoid blocking if no device
                while rx.recv().is_ok() {}
            }
        }
    });
    tx
});

/// Request the audio thread to play the notification sound.
/// Muting silences phase notifications in both desktop and terminal modes.
pub fn play_notification_sound() {
    if is_muted() {
        return;
    }
    // Ignore send errors silently (e.g., thread exited)
    let _ = AUDIO_TX.send(());
}

/// Fire-and-forget desktop notification (Windows toast / macOS / Linux).
/// Never blocks the TUI and never fails loudly: errors are ignored and the
/// embedded notification sound remains the primary signal.
pub fn desktop_notify(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .show();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_helpers_dont_panic() {
        set_muted(true);
        play_notification_sound();
        desktop_notify("pomodoro test", "notification test — no action needed");
        set_muted(false);
    }
}
