use once_cell::sync::Lazy;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::mpsc;
use std::thread;

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

/// Request the audio thread to play the notification sound
pub fn play_notification_sound() {
    // Ignore send errors silently (e.g., thread exited)
    let _ = AUDIO_TX.send(());
}
