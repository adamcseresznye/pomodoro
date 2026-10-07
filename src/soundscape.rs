//! Offline background audio. The callback owns sample generation; UI updates
//! only replace control values. Device access and decoding happen off the UI thread.
use crate::{config::Config, event::PomodoroTask};
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use std::{
    io::Cursor,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub const COUNT: usize = 8;
pub const RATE: u32 = 24_000;
pub const NAMES: [&str; COUNT] = [
    "Rain",
    "Thunder",
    "Rainforest",
    "Birds",
    "Ocean",
    "Pink noise",
    "Brown noise",
    "Alpha tones",
];
pub const DESCRIPTIONS: [&str; COUNT] = [
    "Steady summer rainfall",
    "Occasional rolling thunder",
    "Tropical insects and forest ambience",
    "Morning birdsong",
    "Waves washing onto the shore",
    "Soft, balanced noise",
    "Deep, mellow noise",
    "200 / 210 Hz stereo tones · 10 Hz difference",
];
pub const PRESETS: [(&str, [f32; COUNT]); 8] = [
    ("Gentle rain", [0.75, 0., 0., 0., 0., 0., 0., 0.]),
    ("Thunderstorm", [0.7, 0.45, 0., 0., 0., 0., 0., 0.]),
    ("Rainforest", [0.15, 0., 0.75, 0.25, 0., 0., 0., 0.]),
    ("Morning birds", [0., 0., 0., 0.8, 0., 0., 0., 0.]),
    ("Ocean", [0., 0., 0., 0., 0.8, 0., 0., 0.]),
    ("Pink noise", [0., 0., 0., 0., 0., 0.75, 0., 0.]),
    ("Brown noise", [0., 0., 0., 0., 0., 0., 0.75, 0.]),
    ("Alpha tones", [0., 0., 0., 0., 0., 0.1, 0., 0.55]),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub levels: [f32; COUNT],
    pub master: f32,
    pub enabled: bool,
    pub during_breaks: bool,
    pub preset: Option<usize>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            levels: PRESETS[0].1,
            master: 0.65,
            enabled: true,
            during_breaks: true,
            preset: Some(0),
        }
    }
}
impl Settings {
    pub fn sanitize(&mut self) {
        fn safe(v: f32) -> f32 {
            if v.is_finite() {
                v.clamp(0., 1.)
            } else {
                0.
            }
        }
        self.master = safe(self.master);
        self.levels = self.levels.map(safe);
        if self.preset.is_some_and(|i| i >= PRESETS.len()) {
            self.preset = None;
        }
    }
    pub fn choose(&mut self, index: usize) {
        if let Some((_, levels)) = PRESETS.get(index) {
            self.levels = *levels;
            self.preset = Some(index);
        }
    }
    pub fn gains(&self, playing: bool, preview: Option<usize>) -> [f32; COUNT] {
        if let Some(index) = preview {
            let mut gains = [0.; COUNT];
            if index < COUNT {
                gains[index] = self.master * 0.75;
            }
            return gains;
        }
        if playing && self.enabled {
            self.levels.map(|v| v * self.master)
        } else {
            [0.; COUNT]
        }
    }
}

pub fn should_play(active: bool, paused: bool, phase: PomodoroTask, during_breaks: bool) -> bool {
    active && !paused && (phase == PomodoroTask::Work || during_breaks)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub version: u32,
    pub session: Config,
    pub sound: Settings,
    pub appearance: crate::appearance::Appearance,
    pub experience: crate::experience::Experience,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            session: Config::default(),
            sound: Settings::default(),
            appearance: crate::appearance::Appearance::default(),
            experience: crate::experience::Experience::default(),
        }
    }
}
impl Preferences {
    pub fn sanitize(&mut self) {
        self.sound.sanitize();
        for atmosphere in &mut self.experience.atmospheres {
            atmosphere.sound.sanitize();
        }
        self.session.work_mins = self.session.work_mins.clamp(1, 180);
        self.session.short_mins = self.session.short_mins.clamp(1, 180);
        self.session.long_mins = self.session.long_mins.clamp(1, 180);
        self.session.cycles = self.session.cycles.clamp(1, 12);
    }
    pub fn load(path: &std::path::Path) -> Self {
        let mut value: Self = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        value.sanitize();
        value
    }
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // A temporary file prevents partial JSON from replacing saved settings.
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, path)
    }
}
pub fn preferences_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("pomodoro-soundscapes").join("preferences.json")
}

#[derive(Clone, Copy)]
struct Control {
    gains: [f32; COUNT],
    restart: Option<usize>,
}

pub struct Engine {
    control: Arc<Mutex<Control>>,
    stop: Arc<AtomicBool>,
    pub status: Arc<Mutex<String>>,
}
impl Engine {
    #[cfg_attr(test, allow(dead_code))]
    pub fn new() -> Self {
        let engine = Self {
            control: Arc::new(Mutex::new(Control {
                gains: [0.; COUNT],
                restart: None,
            })),
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new("Preparing audio…".into())),
        };
        let control = engine.control.clone();
        let stop = engine.stop.clone();
        let status = engine.status.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let (_stream, handle) =
                    OutputStream::try_default().map_err(|e| format!("Audio unavailable: {e}"))?;
                let clips = decode_clips()?;
                let sink = Sink::try_new(&handle).map_err(|e| format!("Audio unavailable: {e}"))?;
                sink.append(Mixer::new(clips, control, stop.clone()));
                *status.lock().unwrap() = "Ready · offline audio".into();
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(25));
                }
                sink.stop();
                Ok(())
            })();
            if let Err(error) = result {
                *status.lock().unwrap() = error;
            }
        });
        engine
    }
    #[cfg(test)]
    pub fn silent() -> Self {
        Self {
            control: Arc::new(Mutex::new(Control {
                gains: [0.; COUNT],
                restart: None,
            })),
            stop: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new("Test audio".into())),
        }
    }
    pub fn update(&self, gains: [f32; COUNT]) {
        if let Ok(mut control) = self.control.lock() {
            control.gains = gains.map(|v| if v.is_finite() { v.clamp(0., 1.) } else { 0. });
        }
    }
    pub fn restart(&self, layer: Option<usize>) {
        if let Ok(mut control) = self.control.lock() {
            control.restart = Some(layer.unwrap_or(COUNT));
        }
    }
    pub fn message(&self) -> String {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "Audio unavailable".into())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

const RECORDINGS: [&[u8]; 5] = [
    include_bytes!("../assets/sounds/rain.ogg"),
    include_bytes!("../assets/sounds/thunder.ogg"),
    include_bytes!("../assets/sounds/rainforest.ogg"),
    include_bytes!("../assets/sounds/birds.ogg"),
    include_bytes!("../assets/sounds/ocean.ogg"),
];
fn decode_clips() -> Result<Vec<Vec<f32>>, String> {
    RECORDINGS
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            let decoder = Decoder::new(Cursor::new(*bytes))
                .map_err(|e| format!("Could not load {}: {e}", NAMES[i]))?;
            if decoder.channels() != 2 || decoder.sample_rate() != RATE {
                return Err(format!("Invalid audio format: {}", NAMES[i]));
            }
            let mut samples: Vec<f32> = decoder.convert_samples().collect();
            if samples.len() < RATE as usize * 2 {
                return Err(format!("Recording too short: {}", NAMES[i]));
            }
            // Thunder has its own quiet interval; its event is gently tapered.
            if i == 1 {
                let fade = (RATE as usize / 2 * 2).min(samples.len() / 4);
                let length = samples.len();
                for n in 0..fade {
                    let a = n as f32 / fade as f32;
                    samples[n] *= a;
                    samples[length - fade + n] *= 1. - a;
                }
                samples.resize(samples.len() + RATE as usize * 2 * 22, 0.);
            } else {
                samples = crossfade_loop(samples, RATE as usize * 2 * 2);
            }
            Ok(samples)
        })
        .collect()
}

/// Overlap the tail and head with a linear crossfade. Playback starts after
/// the consumed head and wraps to that same position, with no silent padding.
fn crossfade_loop(samples: Vec<f32>, overlap: usize) -> Vec<f32> {
    let overlap = (overlap.min(samples.len() / 4) / 2) * 2;
    if overlap == 0 {
        return samples;
    }
    let mut result = samples[overlap..samples.len() - overlap].to_vec();
    let frames = overlap / 2;
    for frame in 0..frames {
        let alpha = frame as f32 / frames as f32;
        for channel in 0..2 {
            let offset = frame * 2 + channel;
            result.push(
                samples[samples.len() - overlap + offset] * (1. - alpha) + samples[offset] * alpha,
            );
        }
    }
    result
}

struct Noise {
    seed: u64,
    pink: [f32; 7],
    brown: f32,
}
impl Noise {
    fn new(seed: u64) -> Self {
        Self {
            seed,
            pink: [0.; 7],
            brown: 0.,
        }
    }
    fn white(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed as u32 as f32 / u32::MAX as f32) * 2. - 1.
    }
    fn pink(&mut self) -> f32 {
        let white = self.white();
        let b = &mut self.pink;
        b[0] = 0.99886 * b[0] + white * 0.0555179;
        b[1] = 0.99332 * b[1] + white * 0.0750759;
        b[2] = 0.969 * b[2] + white * 0.153852;
        b[3] = 0.8665 * b[3] + white * 0.3104856;
        b[4] = 0.55 * b[4] + white * 0.5329522;
        b[5] = -0.7616 * b[5] - white * 0.016898;
        let output = (b.iter().sum::<f32>() + white * 0.5362) * 0.055;
        b[6] = white * 0.115926;
        output
    }
    fn brown(&mut self) -> f32 {
        self.brown = (self.brown + 0.025 * self.white()) / 1.002;
        self.brown * 0.6
    }
}

struct Mixer {
    clips: Vec<Vec<f32>>,
    positions: [usize; 5],
    pending_restart: [bool; 5],
    gains: [f32; COUNT],
    targets: [f32; COUNT],
    control: Arc<Mutex<Control>>,
    stop: Arc<AtomicBool>,
    noise: [Noise; 2],
    frame: u64,
    right: Option<f32>,
}
impl Mixer {
    fn new(clips: Vec<Vec<f32>>, control: Arc<Mutex<Control>>, stop: Arc<AtomicBool>) -> Self {
        Self {
            clips,
            positions: [0; 5],
            pending_restart: [false; 5],
            gains: [0.; COUNT],
            targets: [0.; COUNT],
            control,
            stop,
            noise: [Noise::new(0x73813265), Noise::new(0x91683751)],
            frame: 0,
            right: None,
        }
    }
    fn stereo(&mut self) -> [f32; 2] {
        if self.frame % 240 == 0 {
            if let Ok(mut control) = self.control.lock() {
                self.targets = control.gains;
                if let Some(index) = control.restart.take() {
                    if index < 5 {
                        self.pending_restart[index] = true;
                    } else if index == COUNT {
                        self.pending_restart = [true; 5];
                    }
                }
            }
        }
        // About 250 ms to approach each gain; filters prevent clicks on edits.
        for i in 0..COUNT {
            let target = if i < 5 && self.pending_restart[i] {
                0.
            } else {
                self.targets[i]
            };
            self.gains[i] += (target - self.gains[i]) * 0.0005;
            if i < 5 && self.pending_restart[i] && self.gains[i] < 0.0001 {
                self.positions[i] = 0;
                self.pending_restart[i] = false;
            }
        }
        let mut output = [0.; 2];
        for i in 0..5 {
            let clip = &self.clips[i];
            if !clip.is_empty() {
                let p = self.positions[i];
                output[0] += clip[p] * self.gains[i];
                output[1] += clip[p + 1] * self.gains[i];
                self.positions[i] = (p + 2) % clip.len();
            }
        }
        for (channel, value) in output.iter_mut().enumerate() {
            if self.gains[5] > 0.00001 {
                *value += self.noise[channel].pink() * self.gains[5];
            }
            if self.gains[6] > 0.00001 {
                *value += self.noise[channel].brown() * self.gains[6];
            }
            if self.gains[7] > 0.00001 {
                let frequency = if channel == 0 { 200.0 } else { 210.0 };
                let phase = std::f64::consts::TAU * frequency * (self.frame % RATE as u64) as f64
                    / RATE as f64;
                *value += phase.sin() as f32 * 0.15 * self.gains[7];
            }
            // Boost the quiet recordings; smoothly bound peaks in layered mixes.
            // This also raises volume for users with existing saved settings.
            *value = (*value * 5.0).tanh() * 0.95;
        }
        self.frame = self.frame.wrapping_add(1);
        output
    }
}
impl Iterator for Mixer {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.stop.load(Ordering::Relaxed) {
            return None;
        }
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let stereo = self.stereo();
        self.right = Some(stereo[1]);
        Some(stereo[0])
    }
}
impl Source for Mixer {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mixer(gains: [f32; COUNT]) -> Mixer {
        Mixer::new(
            vec![vec![0.; 400]; 5],
            Arc::new(Mutex::new(Control {
                gains,
                restart: None,
            })),
            Arc::new(AtomicBool::new(false)),
        )
    }
    #[test]
    #[ignore = "requires a system audio device"]
    fn audio_device_smoke() {
        let engine = Engine::new();
        for _ in 0..100 {
            let message = engine.message();
            if message.starts_with("Ready") {
                return;
            }
            assert!(!message.starts_with("Audio unavailable"), "{message}");
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("audio initialization timed out: {}", engine.message());
    }

    #[test]
    fn crossfade_keeps_channels_and_wraps_without_a_step() {
        let source: Vec<_> = (0..2000)
            .flat_map(|frame| {
                let sample = (frame as f32 * 0.02).sin();
                [sample, -sample]
            })
            .collect();
        let result = crossfade_loop(source, 400);
        assert_eq!(result.len(), 3600);
        assert!(result
            .chunks_exact(2)
            .all(|p| (p[0] + p[1]).abs() < 0.00001));
        assert!((result[result.len() - 2] - result[0]).abs() < 0.04);
    }

    #[test]
    fn clips_decode_and_loops_are_stereo() {
        let clips = decode_clips().unwrap();
        assert_eq!(clips.len(), 5);
        for clip in clips {
            assert_eq!(clip.len() % 2, 0);
            assert!(clip.iter().all(|v| v.is_finite()));
            assert!(clip.iter().any(|v| v.abs() > 0.001));
        }
    }
    #[test]
    fn playback_follows_timer_and_preview_is_independent() {
        assert!(should_play(true, false, PomodoroTask::Work, false));
        assert!(!should_play(true, true, PomodoroTask::Work, true));
        assert!(!should_play(false, false, PomodoroTask::Work, true));
        assert!(!should_play(true, false, PomodoroTask::ShortBreak, false));
        assert!(should_play(true, false, PomodoroTask::LongBreak, true));
        let settings = Settings::default();
        assert_eq!(settings.gains(false, None), [0.; COUNT]);
        let preview = settings.gains(false, Some(3));
        assert!(preview[3] > 0.);
        assert_eq!(preview.iter().filter(|v| **v > 0.).count(), 1);
    }
    #[test]
    fn synthesis_is_audible_bounded_and_alpha_is_stereo() {
        for index in 5..COUNT {
            let mut gains = [0.; COUNT];
            gains[index] = 1.;
            let output: Vec<_> = mixer(gains).take(RATE as usize * 2).collect();
            assert!(output.iter().all(|v| v.is_finite() && v.abs() <= 0.95));
            assert!(output.iter().any(|v| v.abs() > 0.01));
            if index == 7 {
                assert!(output.chunks_exact(2).any(|p| (p[0] - p[1]).abs() > 0.03));
            }
        }
    }
    #[test]
    fn quiet_recordings_get_a_substantial_boost_without_overflow() {
        let mut gains = [0.; COUNT];
        gains[0] = 0.35 * 0.75; // Existing users' original rain preset.
        let mut source = mixer(gains);
        source.clips[0] = vec![0.05; 400];
        for _ in 0..24000 {
            source.stereo();
        }
        let level = source.stereo()[0];
        assert!(level > 0.06, "quiet recording wasn't boosted: {level}");
        source.control.lock().unwrap().gains = [1.; COUNT];
        source.clips = vec![vec![1.; 400]; 5];
        assert!(source
            .take(48000)
            .all(|sample| sample.is_finite() && sample.abs() <= 0.95));
    }
    #[test]
    fn gain_changes_fade_instead_of_clicking() {
        let mut gains = [0.; COUNT];
        gains[7] = 1.;
        let mut source = mixer(gains);
        source.stereo();
        assert!(source.gains[7] < 0.001);
        for _ in 0..6000 {
            source.stereo();
        }
        assert!(source.gains[7] > 0.94);
        source.control.lock().unwrap().gains = [0.; COUNT];
        for _ in 0..12000 {
            source.stereo();
        }
        assert!(source.gains[7] < 0.004);
    }
    #[test]
    fn preferences_round_trip_and_corrupt_values_recover() {
        let directory =
            std::env::temp_dir().join(format!("pomodoro-settings-test-{}", std::process::id()));
        let path = directory.join("settings.json");
        let mut prefs = Preferences::default();
        prefs.sound.choose(2);
        prefs.sound.master = 0.6;
        prefs.save(&path).unwrap();
        assert_eq!(prefs, Preferences::load(&path));
        prefs.sound.master = 0.4;
        prefs.save(&path).unwrap();
        assert_eq!(prefs, Preferences::load(&path));
        std::fs::write(&path, b"broken").unwrap();
        assert_eq!(Preferences::load(&path), Preferences::default());
        prefs.sound.levels[0] = f32::NAN;
        prefs.sound.master = 3.;
        prefs.session.cycles = 0;
        prefs.sanitize();
        assert_eq!(prefs.sound.levels[0], 0.);
        assert_eq!(prefs.sound.master, 1.);
        assert_eq!(prefs.session.cycles, 1);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
