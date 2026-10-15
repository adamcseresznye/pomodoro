//! "Your own audio": a direct audio link or local file that plays as an extra
//! mixer layer. Streaming sites (YouTube, Spotify, ...) can't be decoded here,
//! so those links open in the browser instead.
use crate::soundscape::RATE;
use rodio::{Decoder, Source};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Longest stretch kept in memory (stereo samples at `RATE`).
const MAX_SECONDS: usize = 5 * 60;
const MAX_DOWNLOAD_BYTES: &str = "60000000";

#[derive(Debug, PartialEq)]
pub enum Link {
    /// A streaming/page link that has to play in the browser.
    Browser(String),
    /// A direct link to an audio file.
    Download(String),
    /// A file on this computer.
    File(PathBuf),
    Invalid,
}

const BROWSER_HOSTS: [&str; 8] = [
    "youtube.com",
    "youtu.be",
    "spotify.com",
    "soundcloud.com",
    "music.apple.com",
    "deezer.com",
    "tidal.com",
    "twitch.tv",
];

pub fn classify(input: &str) -> Link {
    let text = input.trim().trim_matches('"').trim();
    if text.is_empty() {
        return Link::Invalid;
    }
    let lower = text.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = host.split('@').last().unwrap_or(host);
        let host = host.split(':').next().unwrap_or(host);
        if BROWSER_HOSTS
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
        {
            return Link::Browser(text.to_owned());
        }
        return Link::Download(text.to_owned());
    }
    if lower.contains("://") {
        return Link::Invalid;
    }
    Link::File(PathBuf::from(text))
}

/// Short name for the status line.
pub fn label(input: &str) -> String {
    let text = input.trim().trim_matches('"');
    let name = text
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(text);
    name.split('?')
        .next()
        .unwrap_or(name)
        .chars()
        .take(48)
        .collect()
}

/// Download (if needed), decode and prepare a seamless stereo loop.
pub fn load(source: &Link) -> Result<Vec<f32>, String> {
    match source {
        Link::File(path) => decode_file(path),
        Link::Download(url) => {
            let path = std::env::temp_dir().join(format!(
                "pomodoro-custom-{}-{}.audio",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            ));
            let result = download(url, &path).and_then(|_| decode_file(&path));
            let _ = std::fs::remove_file(&path);
            result
        }
        _ => Err("This link can't be played inside the app".into()),
    }
}

fn download(url: &str, path: &Path) -> Result<(), String> {
    // curl ships with Windows 10+, macOS and most Linux systems.
    let status = Command::new("curl")
        .args(["--location", "--fail", "--silent", "--show-error"])
        .args(["--max-time", "90", "--max-filesize", MAX_DOWNLOAD_BYTES])
        .arg("--output")
        .arg(path)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| "Downloading needs curl, which wasn't found".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(
            "Couldn't download that link. Use a direct link to an MP3, OGG, WAV or FLAC file"
                .into(),
        )
    }
}

fn decode_file(path: &Path) -> Result<Vec<f32>, String> {
    let file = File::open(path).map_err(|_| "Couldn't open that file".to_string())?;
    let decoder = Decoder::new(BufReader::new(file))
        .map_err(|_| "That isn't a supported audio file (use MP3, OGG, WAV or FLAC)".to_string())?;
    let channels = decoder.channels().max(1) as usize;
    let rate = decoder.sample_rate().max(1);
    let limit = rate as usize * channels * MAX_SECONDS;
    let samples: Vec<f32> = decoder.convert_samples().take(limit).collect();
    let stereo = to_stereo_at_rate(&samples, channels, rate);
    if stereo.len() < RATE as usize * 2 * 2 {
        return Err("That audio is too short to loop (needs at least 2 seconds)".into());
    }
    Ok(crate::soundscape::crossfade_loop(
        stereo,
        RATE as usize * 2 * 2,
    ))
}

/// Mix any channel count down to stereo and resample linearly to `RATE`.
fn to_stereo_at_rate(samples: &[f32], channels: usize, rate: u32) -> Vec<f32> {
    let frames: Vec<[f32; 2]> = samples
        .chunks_exact(channels)
        .map(|f| match channels {
            1 => [f[0], f[0]],
            _ => [f[0], f[1]],
        })
        .collect();
    if frames.is_empty() {
        return Vec::new();
    }
    let out_frames = (frames.len() as u64 * RATE as u64 / rate as u64) as usize;
    let step = rate as f64 / RATE as f64;
    let mut out = Vec::with_capacity(out_frames * 2);
    for n in 0..out_frames {
        let position = n as f64 * step;
        let i = position as usize;
        let t = (position - i as f64) as f32;
        let a = frames[i.min(frames.len() - 1)];
        let b = frames[(i + 1).min(frames.len() - 1)];
        out.push(a[0] + (b[0] - a[0]) * t);
        out.push(a[1] + (b[1] - a[1]) * t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_sites_open_in_the_browser() {
        for url in [
            "https://www.youtube.com/watch?v=abc",
            "https://youtu.be/abc",
            "https://music.youtube.com/watch?v=abc",
            "https://open.spotify.com/playlist/xyz",
            "http://soundcloud.com/a/b",
        ] {
            assert!(matches!(classify(url), Link::Browser(_)), "{url}");
        }
    }

    #[test]
    fn direct_links_files_and_junk_are_told_apart() {
        assert_eq!(
            classify(" https://example.com/lofi.mp3 "),
            Link::Download("https://example.com/lofi.mp3".into())
        );
        // A look-alike host is not a streaming site.
        assert!(matches!(
            classify("https://notyoutube.com/a.mp3"),
            Link::Download(_)
        ));
        assert_eq!(
            classify("\"C:\\Music\\cafe.ogg\""),
            Link::File(PathBuf::from("C:\\Music\\cafe.ogg"))
        );
        assert_eq!(classify("ftp://example.com/a.mp3"), Link::Invalid);
        assert_eq!(classify("   "), Link::Invalid);
    }

    #[test]
    fn a_local_wav_file_loads_as_a_stereo_loop() {
        let rate = 8000u32;
        let data: Vec<u8> = (0..rate * 4)
            .flat_map(|n| (((n as f32 * 0.05).sin() * 8000.0) as i16).to_le_bytes())
            .collect();
        let mut wav = b"RIFF".to_vec();
        wav.extend((36 + data.len() as u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes()); // PCM
        wav.extend(1u16.to_le_bytes()); // mono
        wav.extend(rate.to_le_bytes());
        wav.extend((rate * 2).to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((data.len() as u32).to_le_bytes());
        wav.extend(data);
        let path =
            std::env::temp_dir().join(format!("pomodoro-custom-test-{}.wav", std::process::id()));
        std::fs::write(&path, wav).unwrap();
        let clip = load(&Link::File(path.clone())).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(clip.len() % 2, 0);
        assert!(clip.len() > RATE as usize * 2 * 2);
        assert!(clip.iter().all(|v| v.is_finite()));
        assert!(clip.iter().any(|v| v.abs() > 0.05));
        assert!(load(&Link::File("nope.mp3".into())).is_err());
    }

    #[test]
    fn labels_use_the_file_name() {
        assert_eq!(label("https://example.com/a/lofi.mp3?x=1"), "lofi.mp3");
        assert_eq!(label("C:\\Music\\cafe.ogg"), "cafe.ogg");
    }

    #[test]
    fn mono_and_other_rates_become_stereo_at_the_engine_rate() {
        let mono: Vec<f32> = (0..4800).map(|n| (n as f32 * 0.01).sin()).collect();
        let out = to_stereo_at_rate(&mono, 1, 48_000);
        assert_eq!(out.len(), 2400 * 2);
        assert!(out.chunks_exact(2).all(|p| p[0] == p[1]));
    }
}
