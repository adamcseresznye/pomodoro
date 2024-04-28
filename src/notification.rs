use rodio::{source::Source, Decoder, OutputStream};
use std::io::Cursor;
use std::thread;
use std::time::Duration;

pub fn play_notification_sound() {
    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    // Decode that sound file into a source
    let sound_data = include_bytes!("bell.mp3");
    let cursor = Cursor::new(sound_data);
    let source = Decoder::new(cursor).unwrap();
    // Play the sound directly on the device
    match stream_handle.play_raw(source.convert_samples()) {
        Ok(_) => {}
        Err(er) => eprintln!("Could not convert audio due to {}", er),
    }

    // The sound plays in a separate audio thread,
    // so we need to keep the main thread alive while it's playing.
    thread::sleep(Duration::from_secs(1));
}
