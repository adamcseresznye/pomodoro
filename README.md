# Pomodoro Soundscapes

A Rust desktop timer built with egui and eframe. A clear countdown, clickable
controls, and an offline soundscape mixer help you set your atmosphere.
The original terminal interface is also available.

The whole app is the clock: a large progress ring with your focus, pause and
skip controls. Your soundscape and session settings sit behind two buttons under
the clock, so nothing competes with the timer. Manrope is bundled for offline
typography.

![Pomodoro desktop app showing the focus clock](assets/screenshot.png)

- Focus, short-break and long-break timers with a progress ring and centered controls.
- Eight sounds to combine: rain, thunder, rainforest, birds, ocean, pink noise,
  brown noise and alpha tones.
- Independent layer levels, master volume, eight presets and audio previews.
- Soundscape and session settings one click away, in sheets over the clock.
- Saved preferences, keyboard shortcuts and optional desktop notifications.
- Offline playback with bundled recordings and generated noise and tones.

## Download

Get the file for your system from the
[latest release](https://github.com/adamcseresznye/pomodoro/releases/latest).
Fonts and soundscapes are bundled, so nothing else needs installing.

| System | File | How to open it |
|---|---|---|
| Windows 10/11 (64-bit) | `pomodoro-soundscapes-windows-x64.msi` | Double-click it to install; adds a Start menu shortcut. |
| Windows, no install | `pomodoro-soundscapes-windows-x64.exe` | Double-click it to run directly. |
| macOS, Apple Silicon (M1 or later) | `pomodoro-soundscapes-macos-arm64.dmg` | Drag the app to Applications. |
| macOS, Intel | `pomodoro-soundscapes-macos-x64.dmg` | Drag the app to Applications. |
| Linux, Debian/Ubuntu (64-bit) | `pomodoro-soundscapes_<version>_amd64.deb` | `sudo apt install ./pomodoro-soundscapes_*_amd64.deb` |
| Linux, other (64-bit) | `pomodoro-soundscapes-linux-x64.tar.gz` | Extract it and run `pomodoro-soundscapes`. |

Each file has a matching `.sha256` checksum.

The apps are not code-signed, so your system may warn the first time:
- **Windows:** SmartScreen says "Windows protected your PC". Choose **More info**, then **Run anyway**.
- **macOS:** if the app is blocked, right-click it, choose **Open**, then confirm. Or allow it under
  **System Settings > Privacy & Security**.
- **Linux:** the tarball may need `chmod +x pomodoro-soundscapes`. Audio uses ALSA (PipeWire and
  PulseAudio provide it), and a desktop notification service is used if one is running.

## Build requirements

Install [Rust](https://www.rust-lang.org/tools/install) with Cargo.
On Windows, use the MSVC toolchain with Visual Studio Build Tools (Desktop
development with C++) and the Windows SDK. The SDK resource compiler embeds
the app icon into the executable. macOS needs only the Xcode command line tools.

Linux builds also need development libraries for ALSA and the window system.
On Ubuntu/Debian:

```sh
sudo apt install build-essential pkg-config libasound2-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev libwayland-dev libxkbcommon-dev libgl1-mesa-dev
```

## Run

```powershell
git clone https://github.com/adamcseresznye/pomodoro.git
cd pomodoro
cargo run
cargo run -- --work 50 --short 10 --long 30 --cycles 3
cargo run -- --terminal
cargo build --release
```

Open `target/release/pomodoro` (`pomodoro.exe` on Windows) to launch the desktop window. Change focus,
break lengths, and session count before starting. Choose a soundscape preset or mix your own background audio. **Soundscape**, under the clock, opens the mixer.

**Settings**, under the clock, opens the session settings. Set focus and
break lengths, the number of sessions, automatic phase starts and desktop
notifications. Changes made during a running session are saved for the next
session; the current countdown continues unchanged. On smaller windows, scroll
to reach the remaining settings.

Opening the desktop app always waits for **Start focusing**, including when
automatic phase starts are enabled. That setting applies to transitions within
a session; **New session** also returns to the waiting screen.

Pause, resume, skip, restart, end a session, and control background audio and the bell independently.
A finished session shows totals and offers a new session. Timing continues
while the desktop window is minimized. Sound and desktop notifications mark
phase changes. Skipped phases do not add to completed-session totals.

## Make it your space

Use **Appearance** for dark mode, the clock's focus and break colors (pick a pair
or set your own), custom backgrounds, gradients, and grain, linen, or dot textures. Background colors adjust for readable text. **Atmospheres** pairs
backgrounds with sound mixes: try Rainy evening, Forest morning, or Ocean calm,
or name and save your own. Save with the same name to update an atmosphere.

Add a session intention before starting. **Focus view** keeps the clock,
intention, and essential controls visible; **Show everything** restores settings.
Daily totals show completed focus sessions and minutes, saved across launches
and grouped by your computer's local date. Skips do not count. Audio eases in
and out over two seconds when playback or the mix changes.

## Keyboard shortcuts

| Key | Action |
|---|---|
| Space / Enter | Start, pause, resume, or continue |
| P / R | Pause / resume |
| S / N | Skip phase |
| X | Restart phase |
| M | Toggle notification bell |
| Q / Escape | End session; close from the summary |

## Command-line options

| Option | Purpose |
|---|---|
| `--work MIN` | Focus length, 1–180 minutes (default 25) |
| `--short MIN` | Short break, 1–180 minutes (default 5) |
| `--long MIN` | Long break, 1–180 minutes (default 15) |
| `--cycles N` | Sessions before a long break, 1–12 (default 4) |
| `--auto` | Start immediately and advance phases automatically |
| `--muted` / `--no-sound` | Start with the notification bell off |
| `--no-desktop` | Disable desktop notifications |
| `--terminal` | Use the original terminal interface |
| `--help` | Show command-line help |

## Validation

```powershell
cargo test
```

The desktop window and Windows executable use the icon in `assets/app-icon.png`
and `assets/app-icon.ico`; the vector design is in `assets/app-icon.svg`.

Optional developer screenshot build: `cargo build --features ui-preview`.
Set `EFRAME_SCREENSHOT_TO` to a PNG path before launching that build to capture
the initial window and exit. The ordinary release build does not enable this.

## Soundscapes

The desktop interface offers rain, thunder, rainforest, birds, ocean, pink
noise, brown noise and alpha tones. Choose a preset, adjust each layer and
master volume, or preview a layer or the full mix for eight seconds.
The default master volume is 65%. Output includes a gain boost for the ambient
recordings and smooth peak limiting when multiple layers are combined.

Playback follows the focus timer and pauses when the timer pauses. Select
Continue soundscape during breaks to keep listening during break phases.
Background audio and the notification bell have independent controls.
Recordings are embedded in the executable: no account or network is needed.
Preset, layer levels, master volume, playback options, bell and session settings
are saved in `preferences.json` inside a `pomodoro-soundscapes` folder: `%APPDATA%` on Windows,
`~/Library/Application Support` on macOS, and `~/.config` (or `$XDG_CONFIG_HOME`) on Linux.

### Your own audio

Under the mixer, **Your own audio** adds one more layer from a source you choose,
for example a lofi track or a coffee shop recording you are allowed to use.
Paste a direct link to an MP3, OGG, WAV or FLAC file, paste a file path, or drop a
file onto the window. It loops, follows the timer like the other layers, has its own
volume, and is remembered for next time (and saved inside atmospheres).

- Only the first five minutes are used, and downloads are limited to 60 MB.
- Links are downloaded with `curl`, which ships with Windows 10 and later, macOS
  and most Linux systems.
- YouTube, Spotify, SoundCloud and similar links can't be decoded by the app, so they
  open in your browser instead. They play there, outside the app, and the timer
  can't pause them.

Alpha tones play a 200 Hz tone in the left channel and 210 Hz in the right.
Use stereo headphones to hear the two channels separately. This is an audio
option, with no claimed concentration or health effect.

## Audio credits

Rain, thunder, birds and ocean recordings: BigSoundBank, CC0 1.0.
Rainforest: Jungle Sound Thailand Phuket by Amada44, CC BY-SA 3.0.
See [audio licenses](assets/sounds/LICENSES.md) and [exact sources](assets/sounds/sources.json).
Noise and tones are generated directly by the app.

## Releases

The [release workflow](.github/workflows/release.yml) runs when a version tag
such as `v0.5.0` is pushed. It checks formatting, runs the tests on Windows,
macOS and Linux, builds the Windows `.msi` installer and portable `.exe`, macOS `.dmg` files (Apple Silicon and
Intel) and Linux `.deb` and `.tar.gz` packages, and publishes them with SHA-256
checksums on the GitHub release page. The workflow can also be run manually to
produce downloadable build artifacts without publishing.
